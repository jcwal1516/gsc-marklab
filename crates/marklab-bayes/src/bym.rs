pub use crate::areal_contract::BymPosteriorPredictive;
use crate::areal_contract::{
    component_constraints, validate_predictive_and_diagnostics, validate_sampling,
    validate_scalar as validate_areal_scalar, validate_spec as validate_areal_spec, ArealSpec,
};
use crate::is_worker_timeout;

use serde::{Deserialize, Serialize};

use crate::backend_versions::PYTHON_VERSION;
use crate::ClaimStatus;
use crate::{
    model::{
        BackendContract, DiagnosticPolicy, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION,
        PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, NutsSamplingSpec, PosteriorDiagnostics, SamplingSummary,
    SarCoefficientSummary, ScalarSummary, WorkerBackend,
};
use marklab_core::backend::BackendCapability;
use marklab_core::backend::ModelMaturity;

/// Poisson BYM disease-mapping model with separate spatial and independent SDs.
///
/// The likelihood is `count_i ~ Poisson(expected_i * exp(alpha + X_i beta + s_i + u_i))`. The
/// intercept prior is `Normal(intercept_prior_mean, intercept_prior_sd)` and each coefficient
/// has `Normal(0, coefficient_prior_sd)` prior. Independent half-normal priors with the
/// supplied scales govern `sigma_s` and `sigma_u`. The worker uses non-centred effects `s =
/// sigma_s T z` and `u = sigma_u v`, with independent standard-normal `z` and `v`. The
/// supplied ICAR transform `T` constrains the structured effect to sum to zero within each
/// connected component.
///
#[derive(Clone, Debug)]
pub struct BymFitSpec {
    pub region_ids: Vec<String>,
    pub weights_digest_sha256: String,
    pub icar_transform: Vec<f64>,
    pub rank_deficiency: usize,
    pub components: Vec<Vec<usize>>,
    pub counts: Vec<u64>,
    pub expected: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub structured_sd_prior: f64,
    pub unstructured_sd_prior: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BymModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub likelihood: &'static str,
    pub offset: &'static str,
    pub structured_effect: &'static str,
    pub unstructured_effect: &'static str,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub structured_sd_prior: f64,
    pub unstructured_sd_prior: f64,
    pub backend_capability: BackendCapability,
    pub maturity: ModelMaturity,
}

#[derive(Clone, Debug, Serialize)]
pub struct BymFitWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: BymModelIr,
    pub region_ids: Vec<String>,
    pub weights_digest_sha256: String,
    pub icar_transform: Vec<f64>,
    pub rank_deficiency: usize,
    pub components: Vec<Vec<usize>>,
    pub counts: Vec<u64>,
    pub expected: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl BymFitWorkerRequest {
    pub fn new(
        spec: BymFitSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        validate_spec(&spec)?;
        if !is_worker_timeout(timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "BYM worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        let iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        if iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "requested BYM NUTS iterations exceed 400000".into(),
            ));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "pymc",
                version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                environment_lock_sha256,
                worker_sha256,
            },
            model: BymModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "poisson_bym_disease_mapping",
                likelihood: "poisson_log_link",
                offset: "log_expected",
                structured_effect: "exact_sum_to_zero_noncentered_icar",
                unstructured_effect: "iid_standard_normal_scaled",
                intercept_prior_mean: spec.intercept_prior_mean,
                intercept_prior_sd: spec.intercept_prior_sd,
                coefficient_prior_sd: spec.coefficient_prior_sd,
                structured_sd_prior: spec.structured_sd_prior,
                unstructured_sd_prior: spec.unstructured_sd_prior,
                backend_capability: BackendCapability::Nuts,
                maturity: ModelMaturity::Experimental,
            },
            region_ids: spec.region_ids,
            weights_digest_sha256: spec.weights_digest_sha256,
            icar_transform: spec.icar_transform,
            rank_deficiency: spec.rank_deficiency,
            components: spec.components,
            counts: spec.counts,
            expected: spec.expected,
            design: spec.design,
            predictor_names: spec.predictor_names,
            sampling,
            resources: WorkerResourceLimits {
                maximum_observations: 64,
                maximum_total_iterations,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

fn validate_spec(spec: &BymFitSpec) -> Result<(), BayesError> {
    validate_areal_spec(
        ArealSpec {
            region_ids: &spec.region_ids,
            weights_digest_sha256: &spec.weights_digest_sha256,
            transform: &spec.icar_transform,
            rank_deficiency: spec.rank_deficiency,
            components: &spec.components,
            counts_len: spec.counts.len(),
            expected: &spec.expected,
            design: &spec.design,
            predictor_names: &spec.predictor_names,
        },
        "BYM",
    )?;
    for (value, name) in [
        (spec.intercept_prior_sd, "intercept prior SD"),
        (spec.coefficient_prior_sd, "coefficient prior SD"),
        (spec.structured_sd_prior, "structured SD prior"),
        (spec.unstructured_sd_prior, "unstructured SD prior"),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(BayesError::InvalidSpec(format!(
                "BYM {name} must be finite and positive"
            )));
        }
    }
    if !spec.intercept_prior_mean.is_finite() {
        return Err(BayesError::InvalidSpec(
            "BYM intercept prior mean must be finite".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BymPosterior {
    pub intercept: ScalarSummary,
    pub coefficients: Vec<SarCoefficientSummary>,
    pub structured_sd: ScalarSummary,
    pub unstructured_sd: ScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BymRegionSummary {
    pub region_id: String,
    pub observed_count: u64,
    pub expected: f64,
    pub structured_effect: ScalarSummary,
    pub unstructured_effect: ScalarSummary,
    pub relative_risk: ScalarSummary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BymFitWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BymPosterior,
    pub regions: Vec<BymRegionSummary>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: BymPosteriorPredictive,
}

impl BymFitWorkerResult {
    pub fn validate(
        &self,
        request: &BymFitWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly
            || self.format != "marklab.pymc_bym_worker_result"
            || self.version != 1
            || !self.backend.matches(&request.backend)
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "BYM result identity mismatch".into(),
            ));
        }
        validate_sampling(
            &self.sampling,
            &request.sampling,
            self.posterior.coefficients.len(),
            request.predictor_names.len(),
            self.regions.len(),
            request.region_ids.len(),
            "BYM",
        )?;
        for summary in [
            &self.posterior.intercept,
            &self.posterior.structured_sd,
            &self.posterior.unstructured_sd,
        ] {
            validate_scalar(summary, false)?;
        }
        if self.posterior.structured_sd.mean <= 0.0
            || self.posterior.structured_sd.interval_lower < 0.0
            || self.posterior.unstructured_sd.mean <= 0.0
            || self.posterior.unstructured_sd.interval_lower < 0.0
        {
            return Err(BayesError::WorkerContract(
                "BYM scale posterior violates support".into(),
            ));
        }
        for (actual, expected) in self
            .posterior
            .coefficients
            .iter()
            .zip(&request.predictor_names)
        {
            if actual.predictor != *expected {
                return Err(BayesError::WorkerContract(
                    "BYM coefficient identity mismatch".into(),
                ));
            }
            validate_scalar(&actual.summary, false)?;
        }
        for (index, region) in self.regions.iter().enumerate() {
            if region.region_id != request.region_ids[index]
                || region.observed_count != request.counts[index]
                || region.expected.to_bits() != request.expected[index].to_bits()
            {
                return Err(BayesError::WorkerContract(
                    "BYM region result changed exact input identity".into(),
                ));
            }
            validate_scalar(&region.structured_effect, false)?;
            validate_scalar(&region.unstructured_effect, false)?;
            validate_scalar(&region.relative_risk, true)?;
            if region.relative_risk.mean <= 0.0 || region.relative_risk.interval_lower <= 0.0 {
                return Err(BayesError::WorkerContract(
                    "BYM relative risk violates positive support".into(),
                ));
            }
        }
        let pass = validate_predictive_and_diagnostics(
            &self.posterior_predictive,
            &self.diagnostics,
            &request.counts,
            &request.diagnostic_policy,
            "BYM",
        )?;
        if (self.fit_state == FitState::Complete) != pass {
            return Err(BayesError::WorkerContract(
                "BYM fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(self, request: BymFitWorkerRequest, input: BymInputIdentity) -> BymFitResult {
        BymFitResult {
            format: "marklab.bayesian_bym_fit",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            fit_state: self.fit_state,
            claim_status: match self.fit_state {
                FitState::Complete => ClaimStatus::Experimental,
                FitState::Nonconverged => ClaimStatus::DiagnosticOnlyNonconverged,
                FitState::ApproximateOnly => ClaimStatus::ExperimentalApproximateOnly,
            },
            sampling: self.sampling,
            posterior: self.posterior,
            rank_deficiency: request.rank_deficiency,
            constraints: component_constraints(&request.components, &request.region_ids),
            regions: self.regions,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_scalar(summary: &ScalarSummary, positive: bool) -> Result<(), BayesError> {
    validate_areal_scalar(summary, positive, "BYM")
}

#[derive(Debug, Serialize)]
pub struct BymInputIdentity {
    pub regions_path: String,
    pub edges_path: String,
    pub data_path: String,
    pub weights_digest_sha256: String,
    pub data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct BymFitResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: BymModelIr,
    pub input: BymInputIdentity,
    pub fit_state: FitState,
    pub claim_status: ClaimStatus,
    pub sampling: SamplingSummary,
    pub posterior: BymPosterior,
    pub rank_deficiency: usize,
    pub constraints: Vec<String>,
    pub regions: Vec<BymRegionSummary>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: BymPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

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
    BayesError, BymPosteriorPredictive, FitState, NutsSamplingSpec, PosteriorDiagnostics,
    SamplingSummary, SarCoefficientSummary, ScalarSummary, WorkerBackend,
};
use marklab_core::backend::BackendCapability;
use marklab_core::backend::ModelMaturity;

/// Poisson BYM2 model with total scale and a structured-variance mixing fraction.
///
/// The likelihood is `count_i ~ Poisson(expected_i * exp(alpha + X_i beta + b_i))`.
/// Independent priors are `alpha ~ Normal(intercept_prior_mean, intercept_prior_sd)`,
/// `beta_j ~ Normal(0, coefficient_prior_sd)`, `sigma ~ HalfNormal(total_sd_prior)`, and
/// `phi ~ Beta(phi_alpha, phi_beta)`. The non-centred effect is
/// `b = sigma * (sqrt(phi) T z + sqrt(1 - phi) v)` for independent standard-normal vectors.
/// The supplied scaled ICAR transform `T` has unit geometric-mean marginal variance and imposes
/// a sum-zero constraint within each connected component.
#[derive(Clone, Debug)]
pub struct Bym2FitSpec {
    pub region_ids: Vec<String>,
    pub weights_digest_sha256: String,
    pub scaled_icar_transform: Vec<f64>,
    pub rank_deficiency: usize,
    pub components: Vec<Vec<usize>>,
    pub original_typical_marginal_variance: f64,
    pub scaled_typical_marginal_variance: f64,
    pub counts: Vec<u64>,
    pub expected: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub total_sd_prior: f64,
    pub phi_alpha: f64,
    pub phi_beta: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Bym2ModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub likelihood: &'static str,
    pub offset: &'static str,
    pub reparameterization: &'static str,
    pub icar_scaling: &'static str,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub total_sd_prior: f64,
    pub phi_alpha: f64,
    pub phi_beta: f64,
    pub backend_capability: BackendCapability,
    pub maturity: ModelMaturity,
}

#[derive(Clone, Debug, Serialize)]
pub struct Bym2FitWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: Bym2ModelIr,
    pub region_ids: Vec<String>,
    pub weights_digest_sha256: String,
    pub scaled_icar_transform: Vec<f64>,
    pub rank_deficiency: usize,
    pub components: Vec<Vec<usize>>,
    pub original_typical_marginal_variance: f64,
    pub scaled_typical_marginal_variance: f64,
    pub counts: Vec<u64>,
    pub expected: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl Bym2FitWorkerRequest {
    pub fn new(
        spec: Bym2FitSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        validate_spec(&spec)?;
        if !is_worker_timeout(timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "BYM2 worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        let iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        if iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "requested BYM2 NUTS iterations exceed 400000".into(),
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
            model: Bym2ModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "poisson_bym2_disease_mapping",
                likelihood: "poisson_log_link",
                offset: "log_expected",
                reparameterization: "sigma_times_sqrt_phi_icar_plus_sqrt_one_minus_phi_iid",
                icar_scaling: "geometric_mean_generalized_marginal_variance_one",
                intercept_prior_mean: spec.intercept_prior_mean,
                intercept_prior_sd: spec.intercept_prior_sd,
                coefficient_prior_sd: spec.coefficient_prior_sd,
                total_sd_prior: spec.total_sd_prior,
                phi_alpha: spec.phi_alpha,
                phi_beta: spec.phi_beta,
                backend_capability: BackendCapability::Nuts,
                maturity: ModelMaturity::Experimental,
            },
            region_ids: spec.region_ids,
            weights_digest_sha256: spec.weights_digest_sha256,
            scaled_icar_transform: spec.scaled_icar_transform,
            rank_deficiency: spec.rank_deficiency,
            components: spec.components,
            original_typical_marginal_variance: spec.original_typical_marginal_variance,
            scaled_typical_marginal_variance: spec.scaled_typical_marginal_variance,
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

fn validate_spec(spec: &Bym2FitSpec) -> Result<(), BayesError> {
    validate_areal_spec(
        ArealSpec {
            region_ids: &spec.region_ids,
            weights_digest_sha256: &spec.weights_digest_sha256,
            transform: &spec.scaled_icar_transform,
            rank_deficiency: spec.rank_deficiency,
            components: &spec.components,
            counts_len: spec.counts.len(),
            expected: &spec.expected,
            design: &spec.design,
            predictor_names: &spec.predictor_names,
        },
        "BYM2",
    )?;
    if !spec.original_typical_marginal_variance.is_finite()
        || spec.original_typical_marginal_variance <= 0.0
        || !spec.scaled_typical_marginal_variance.is_finite()
        || (spec.scaled_typical_marginal_variance - 1.0).abs() > 1e-12
    {
        return Err(BayesError::InvalidSpec("BYM2 ICAR scale is invalid".into()));
    }
    for (value, name) in [
        (spec.intercept_prior_sd, "intercept prior SD"),
        (spec.coefficient_prior_sd, "coefficient prior SD"),
        (spec.total_sd_prior, "total SD prior"),
        (spec.phi_alpha, "phi alpha"),
        (spec.phi_beta, "phi beta"),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(BayesError::InvalidSpec(format!(
                "BYM2 {name} must be finite and positive"
            )));
        }
    }
    if !spec.intercept_prior_mean.is_finite() {
        return Err(BayesError::InvalidSpec(
            "BYM2 intercept prior mean must be finite".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bym2Posterior {
    pub intercept: ScalarSummary,
    pub coefficients: Vec<SarCoefficientSummary>,
    pub sigma: ScalarSummary,
    pub phi: ScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bym2RegionSummary {
    pub region_id: String,
    pub observed_count: u64,
    pub expected: f64,
    pub structured_component: ScalarSummary,
    pub unstructured_component: ScalarSummary,
    pub combined_effect: ScalarSummary,
    pub relative_risk: ScalarSummary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bym2FitWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: Bym2Posterior,
    pub regions: Vec<Bym2RegionSummary>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: BymPosteriorPredictive,
}

impl Bym2FitWorkerResult {
    pub fn validate(
        &self,
        request: &Bym2FitWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly
            || self.format != "marklab.pymc_bym2_worker_result"
            || self.version != 1
            || !self.backend.matches(&request.backend)
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "BYM2 result identity mismatch".into(),
            ));
        }
        validate_sampling(
            &self.sampling,
            &request.sampling,
            self.posterior.coefficients.len(),
            request.predictor_names.len(),
            self.regions.len(),
            request.region_ids.len(),
            "BYM2",
        )?;
        for summary in [
            &self.posterior.intercept,
            &self.posterior.sigma,
            &self.posterior.phi,
        ] {
            validate_scalar(summary, false)?;
        }
        if self.posterior.sigma.mean <= 0.0
            || self.posterior.sigma.interval_lower < 0.0
            || self.posterior.phi.interval_lower < 0.0
            || self.posterior.phi.interval_upper > 1.0
        {
            return Err(BayesError::WorkerContract(
                "BYM2 sigma or phi posterior violates support".into(),
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
                    "BYM2 coefficient identity mismatch".into(),
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
                    "BYM2 region result changed exact input identity".into(),
                ));
            }
            validate_scalar(&region.structured_component, false)?;
            validate_scalar(&region.unstructured_component, false)?;
            validate_scalar(&region.combined_effect, false)?;
            validate_scalar(&region.relative_risk, true)?;
        }
        let pass = validate_predictive_and_diagnostics(
            &self.posterior_predictive,
            &self.diagnostics,
            &request.counts,
            &request.diagnostic_policy,
            "BYM2",
        )?;
        if (self.fit_state == FitState::Complete) != pass {
            return Err(BayesError::WorkerContract(
                "BYM2 fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: Bym2FitWorkerRequest,
        input: Bym2InputIdentity,
    ) -> Bym2FitResult {
        Bym2FitResult {
            format: "marklab.bayesian_bym2_fit",
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
            icar_scaling: Bym2IcarScaling {
                convention: "geometric_mean_generalized_marginal_variance",
                original_typical_marginal_variance: request.original_typical_marginal_variance,
                scaled_typical_marginal_variance: request.scaled_typical_marginal_variance,
            },
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
    validate_areal_scalar(summary, positive, "BYM2")
}

#[derive(Debug, Serialize)]
pub struct Bym2InputIdentity {
    pub regions_path: String,
    pub edges_path: String,
    pub data_path: String,
    pub weights_digest_sha256: String,
    pub data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Bym2IcarScaling {
    pub convention: &'static str,
    pub original_typical_marginal_variance: f64,
    pub scaled_typical_marginal_variance: f64,
}

#[derive(Debug, Serialize)]
pub struct Bym2FitResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: Bym2ModelIr,
    pub input: Bym2InputIdentity,
    pub fit_state: FitState,
    pub claim_status: ClaimStatus,
    pub sampling: SamplingSummary,
    pub posterior: Bym2Posterior,
    pub icar_scaling: Bym2IcarScaling,
    pub rank_deficiency: usize,
    pub constraints: Vec<String>,
    pub regions: Vec<Bym2RegionSummary>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: BymPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

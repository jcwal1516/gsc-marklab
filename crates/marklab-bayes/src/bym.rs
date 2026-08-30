use crate::validation::all_finite as finite;

use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, DiagnosticPolicy, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION,
        PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    sar_fit::design_full_rank,
    BayesError, FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary,
    SarCoefficientSummary, SarScalarSummary, WorkerBackend,
};

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
    pub backend_capability: &'static str,
    pub maturity: &'static str,
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
        if !(1..=3_600).contains(&timeout_seconds) {
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
                python_version: "3.12",
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
                backend_capability: "nuts",
                maturity: "experimental",
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
    let dimension = spec.region_ids.len();
    let predictors = spec.predictor_names.len();
    let constrained_dimension = dimension.saturating_sub(spec.rank_deficiency);
    if !(6..=64).contains(&dimension)
        || !(1..=16).contains(&predictors)
        || spec.counts.len() != dimension
        || spec.expected.len() != dimension
        || spec.design.len() != dimension * predictors
        || spec.icar_transform.len() != dimension * constrained_dimension
        || spec.rank_deficiency == 0
        || spec.rank_deficiency >= dimension
        || spec.components.len() != spec.rank_deficiency
    {
        return Err(BayesError::InvalidSpec(
            "BYM input dimensions violate the region, predictor, or ICAR contract".into(),
        ));
    }
    for (index, region_id) in spec.region_ids.iter().enumerate() {
        if region_id.is_empty()
            || region_id.trim() != region_id
            || (index > 0 && spec.region_ids[index - 1] >= *region_id)
        {
            return Err(BayesError::InvalidSpec(
                "BYM region IDs must be exact and strictly increasing".into(),
            ));
        }
    }
    for (index, predictor) in spec.predictor_names.iter().enumerate() {
        if predictor.is_empty()
            || predictor.trim() != predictor
            || predictor == "intercept"
            || spec.predictor_names[..index].contains(predictor)
        {
            return Err(BayesError::InvalidSpec(
                "BYM predictor names must be exact, unique, and not intercept".into(),
            ));
        }
    }
    if spec
        .expected
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        || spec.design.iter().any(|value| !value.is_finite())
        || spec.icar_transform.iter().any(|value| !value.is_finite())
        || !design_full_rank(&spec.design, dimension, predictors)
    {
        return Err(BayesError::InvalidSpec(
            "BYM expected counts, design, transform, or design rank are invalid".into(),
        ));
    }
    let mut covered = vec![false; dimension];
    for component in &spec.components {
        if component.len() < 2 {
            return Err(BayesError::InvalidSpec(
                "BYM ICAR components must contain at least two regions".into(),
            ));
        }
        for &index in component {
            if index >= dimension || std::mem::replace(&mut covered[index], true) {
                return Err(BayesError::InvalidSpec(
                    "BYM ICAR components must partition all regions exactly once".into(),
                ));
            }
        }
    }
    if covered.iter().any(|covered| !covered) {
        return Err(BayesError::InvalidSpec(
            "BYM ICAR components do not cover every region".into(),
        ));
    }
    for component in &spec.components {
        for column in 0..constrained_dimension {
            let sum = component
                .iter()
                .map(|&row| spec.icar_transform[row * constrained_dimension + column])
                .sum::<f64>();
            if sum.abs() > 1e-10 {
                return Err(BayesError::InvalidSpec(
                    "BYM ICAR transform violates a component sum-to-zero constraint".into(),
                ));
            }
        }
    }
    if spec.weights_digest_sha256.len() != 64
        || !spec
            .weights_digest_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(BayesError::InvalidSpec(
            "BYM requires a SHA-256 weights identity".into(),
        ));
    }
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
    pub intercept: SarScalarSummary,
    pub coefficients: Vec<SarCoefficientSummary>,
    pub structured_sd: SarScalarSummary,
    pub unstructured_sd: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BymRegionSummary {
    pub region_id: String,
    pub observed_count: u64,
    pub expected: f64,
    pub structured_effect: SarScalarSummary,
    pub unstructured_effect: SarScalarSummary,
    pub relative_risk: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BymPosteriorPredictive {
    pub observed_total_count: u64,
    pub replicated_total_mean: f64,
    pub replicated_total_sd: f64,
    pub observed_zero_regions: u64,
    pub replicated_zero_regions_mean: f64,
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
    pub diagnostics: NormalMeanDiagnostics,
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
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "BYM result identity mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
            || self.posterior.coefficients.len() != request.predictor_names.len()
            || self.regions.len() != request.region_ids.len()
        {
            return Err(BayesError::WorkerContract(
                "BYM sampling, coefficient, or region counts mismatch".into(),
            ));
        }
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
        let observed_total = request.counts.iter().sum::<u64>();
        let observed_zeros = request.counts.iter().filter(|&&count| count == 0).count() as u64;
        if self.posterior_predictive.observed_total_count != observed_total
            || self.posterior_predictive.observed_zero_regions != observed_zeros
            || !finite(&[
                self.posterior_predictive.replicated_total_mean,
                self.posterior_predictive.replicated_total_sd,
                self.posterior_predictive.replicated_zero_regions_mean,
                self.diagnostics.r_hat,
                self.diagnostics.ess_bulk,
                self.diagnostics.ess_tail,
                self.diagnostics.mcse_mean,
                self.diagnostics.mcse_sd,
                self.diagnostics.minimum_ebfmi,
            ])
            || self.posterior_predictive.replicated_total_sd < 0.0
            || self.posterior_predictive.replicated_zero_regions_mean < 0.0
            || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
        {
            return Err(BayesError::WorkerContract(
                "BYM posterior predictive or diagnostics are invalid".into(),
            ));
        }
        let pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= request.diagnostic_policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= request.diagnostic_policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= request.diagnostic_policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= request.diagnostic_policy.minimum_ebfmi
            && self.diagnostics.divergences <= request.diagnostic_policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits
                <= request.diagnostic_policy.maximum_tree_depth_hits;
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
                FitState::Complete => "experimental",
                FitState::Nonconverged => "diagnostic_only_nonconverged",
                FitState::ApproximateOnly => "experimental_approximate_only",
            },
            sampling: self.sampling,
            posterior: self.posterior,
            rank_deficiency: request.rank_deficiency,
            constraints: request
                .components
                .iter()
                .map(|component| {
                    format!(
                        "sum_to_zero:{}",
                        component
                            .iter()
                            .map(|&index| request.region_ids[index].as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect(),
            regions: self.regions,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_scalar(summary: &SarScalarSummary, positive: bool) -> Result<(), BayesError> {
    if !finite(&[
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]) || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
        || (positive && (summary.mean <= 0.0 || summary.interval_lower <= 0.0))
    {
        return Err(BayesError::WorkerContract(
            "invalid BYM scalar summary".into(),
        ));
    }
    Ok(())
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
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: BymPosterior,
    pub rank_deficiency: usize,
    pub constraints: Vec<String>,
    pub regions: Vec<BymRegionSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BymPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

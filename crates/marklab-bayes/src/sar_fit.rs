use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, DiagnosticPolicy, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION,
        PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary, SarModelType,
    WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct SarFitSpec {
    pub model_type: SarModelType,
    pub region_ids: Vec<String>,
    pub weights: Vec<f64>,
    pub weights_digest_sha256: String,
    pub response: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub rho_bound: f64,
    pub sigma_prior_sd: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SarFitModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub model_type: SarModelType,
    pub interpretation: &'static str,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub rho_bound: f64,
    pub sigma_prior_sd: f64,
    pub weight_normalization: &'static str,
    pub likelihood_jacobian: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct SarFitWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: SarFitModelIr,
    pub region_ids: Vec<String>,
    pub weights: Vec<f64>,
    pub weights_digest_sha256: String,
    pub response: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl SarFitWorkerRequest {
    pub fn new(
        spec: SarFitSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        validate_spec(&spec)?;
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        let iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        if iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "requested SAR NUTS iterations exceed 400000".into(),
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
            model: SarFitModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "gaussian_spatial_autoregressive",
                model_type: spec.model_type,
                interpretation: "descriptive",
                intercept_prior_mean: spec.intercept_prior_mean,
                intercept_prior_sd: spec.intercept_prior_sd,
                coefficient_prior_sd: spec.coefficient_prior_sd,
                rho_bound: spec.rho_bound,
                sigma_prior_sd: spec.sigma_prior_sd,
                weight_normalization: "row_standardized_zero_diagonal_island_free",
                likelihood_jacobian: "log_abs_determinant_i_minus_rho_w",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            region_ids: spec.region_ids,
            weights: spec.weights,
            weights_digest_sha256: spec.weights_digest_sha256,
            response: spec.response,
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

fn validate_spec(spec: &SarFitSpec) -> Result<(), BayesError> {
    let dimension = spec.region_ids.len();
    let predictors = spec.predictor_names.len();
    if !(6..=64).contains(&dimension)
        || !(1..=16).contains(&predictors)
        || spec.weights.len() != dimension * dimension
        || spec.response.len() != dimension
        || spec.design.len() != dimension * predictors
    {
        return Err(BayesError::InvalidSpec(
            "SAR fit dimensions exceed the 6-64 region and 1-16 predictor contract".into(),
        ));
    }
    for (index, region_id) in spec.region_ids.iter().enumerate() {
        if region_id.is_empty()
            || region_id.trim() != region_id
            || (index > 0 && spec.region_ids[index - 1] >= *region_id)
        {
            return Err(BayesError::InvalidSpec(
                "SAR fit region IDs must be exact and strictly increasing".into(),
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
                "SAR fit predictor names must be exact, unique, and not intercept".into(),
            ));
        }
    }
    if spec
        .weights
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        || spec.response.iter().any(|value| !value.is_finite())
        || spec.design.iter().any(|value| !value.is_finite())
    {
        return Err(BayesError::InvalidSpec(
            "SAR fit weights, response, and design must be finite and weights nonnegative".into(),
        ));
    }
    if !design_full_rank(&spec.design, dimension, predictors) {
        return Err(BayesError::InvalidSpec(
            "SAR fit design including intercept must have full column rank".into(),
        ));
    }
    for row in 0..dimension {
        if spec.weights[row * dimension + row] != 0.0
            || ((0..dimension)
                .map(|column| spec.weights[row * dimension + column])
                .sum::<f64>()
                - 1.0)
                .abs()
                > 1e-12
        {
            return Err(BayesError::InvalidSpec(
                "SAR fit weights must be zero-diagonal and row standardized without islands".into(),
            ));
        }
    }
    if spec.weights_digest_sha256.len() != 64
        || !spec
            .weights_digest_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(BayesError::InvalidSpec(
            "SAR fit requires a SHA-256 weights identity".into(),
        ));
    }
    for (value, name) in [
        (spec.intercept_prior_sd, "intercept prior SD"),
        (spec.coefficient_prior_sd, "coefficient prior SD"),
        (spec.sigma_prior_sd, "sigma prior SD"),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(BayesError::InvalidSpec(format!(
                "SAR fit {name} must be finite and positive"
            )));
        }
    }
    if !spec.intercept_prior_mean.is_finite()
        || !spec.rho_bound.is_finite()
        || spec.rho_bound <= 0.0
        || spec.rho_bound >= 1.0
    {
        return Err(BayesError::InvalidSpec(
            "SAR fit intercept prior mean and rho bound are invalid".into(),
        ));
    }
    Ok(())
}

pub(crate) fn design_full_rank(design: &[f64], rows: usize, predictors: usize) -> bool {
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(predictors + 1);
    for column in 0..=predictors {
        let mut candidate = if column == 0 {
            vec![1.0; rows]
        } else {
            (0..rows)
                .map(|row| design[row * predictors + column - 1])
                .collect::<Vec<_>>()
        };
        let original_norm = candidate
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        for _ in 0..2 {
            for vector in &basis {
                let projection = candidate
                    .iter()
                    .zip(vector)
                    .map(|(left, right)| left * right)
                    .sum::<f64>();
                for (value, basis_value) in candidate.iter_mut().zip(vector) {
                    *value -= projection * basis_value;
                }
            }
        }
        let residual_norm = candidate
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        if residual_norm <= f64::EPSILON.sqrt() * original_norm.max(1.0) {
            return false;
        }
        for value in &mut candidate {
            *value /= residual_norm;
        }
        basis.push(candidate);
    }
    true
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SarScalarSummary {
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SarCoefficientSummary {
    pub predictor: String,
    #[serde(flatten)]
    pub summary: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SarFitPosterior {
    pub intercept: SarScalarSummary,
    pub coefficients: Vec<SarCoefficientSummary>,
    pub rho: SarScalarSummary,
    pub sigma: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SarImpactSummary {
    pub predictor: String,
    pub direct: SarScalarSummary,
    pub indirect: SarScalarSummary,
    pub total: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SarPosteriorPredictive {
    pub observed_mean: f64,
    pub observed_sd: f64,
    pub replicated_mean_mean: f64,
    pub replicated_mean_sd: f64,
    pub replicated_sd_mean: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SarFitWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: SarFitPosterior,
    pub impacts: Vec<SarImpactSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: SarPosteriorPredictive,
}

impl SarFitWorkerResult {
    pub fn validate(
        &self,
        request: &SarFitWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly
            || self.format != "marklab.pymc_sar_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "SAR fit result identity mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
            || self.posterior.coefficients.len() != request.predictor_names.len()
        {
            return Err(BayesError::WorkerContract(
                "SAR fit sampling or coefficient counts mismatch".into(),
            ));
        }
        validate_scalar(&self.posterior.intercept)?;
        validate_scalar(&self.posterior.rho)?;
        validate_scalar(&self.posterior.sigma)?;
        if self.posterior.sigma.mean <= 0.0
            || self.posterior.sigma.interval_lower < 0.0
            || self.posterior.rho.interval_lower < -request.model.rho_bound
            || self.posterior.rho.interval_upper > request.model.rho_bound
        {
            return Err(BayesError::WorkerContract(
                "SAR fit posterior support is invalid".into(),
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
                    "SAR fit coefficient identity mismatch".into(),
                ));
            }
            validate_scalar(&actual.summary)?;
        }
        let expected_impacts = if request.model.model_type == SarModelType::Lag {
            request.predictor_names.len()
        } else {
            0
        };
        if self.impacts.len() != expected_impacts {
            return Err(BayesError::WorkerContract(
                "SAR fit impact count disagrees with model type".into(),
            ));
        }
        for (impact, predictor) in self.impacts.iter().zip(&request.predictor_names) {
            if impact.predictor != *predictor {
                return Err(BayesError::WorkerContract(
                    "SAR fit impact identity mismatch".into(),
                ));
            }
            validate_scalar(&impact.direct)?;
            validate_scalar(&impact.indirect)?;
            validate_scalar(&impact.total)?;
        }
        let observed_mean = request.response.iter().sum::<f64>() / request.response.len() as f64;
        let observed_sd = (request
            .response
            .iter()
            .map(|value| (value - observed_mean).powi(2))
            .sum::<f64>()
            / (request.response.len() - 1) as f64)
            .sqrt();
        if !finite(&[
            self.diagnostics.r_hat,
            self.diagnostics.ess_bulk,
            self.diagnostics.ess_tail,
            self.diagnostics.mcse_mean,
            self.diagnostics.mcse_sd,
            self.diagnostics.minimum_ebfmi,
            self.posterior_predictive.observed_mean,
            self.posterior_predictive.observed_sd,
            self.posterior_predictive.replicated_mean_mean,
            self.posterior_predictive.replicated_mean_sd,
            self.posterior_predictive.replicated_sd_mean,
        ]) || (self.posterior_predictive.observed_mean - observed_mean).abs()
            > 1e-12 * observed_mean.abs().max(1.0)
            || (self.posterior_predictive.observed_sd - observed_sd).abs()
                > 1e-12 * observed_sd.abs().max(1.0)
            || self.posterior_predictive.replicated_mean_sd < 0.0
            || self.posterior_predictive.replicated_sd_mean < 0.0
            || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
        {
            return Err(BayesError::WorkerContract(
                "SAR fit predictive or diagnostic values are invalid".into(),
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
                "SAR fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: SarFitWorkerRequest,
        input: SarFitInputIdentity,
    ) -> SarFitResult {
        SarFitResult {
            format: "marklab.bayesian_sar_fit",
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
            impacts: self.impacts,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_scalar(summary: &SarScalarSummary) -> Result<(), BayesError> {
    if !finite(&[
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]) || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
    {
        return Err(BayesError::WorkerContract(
            "invalid SAR scalar summary".into(),
        ));
    }
    Ok(())
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[derive(Debug, Serialize)]
pub struct SarFitInputIdentity {
    pub regions_path: String,
    pub edges_path: String,
    pub data_path: String,
    pub weights_digest_sha256: String,
    pub data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct SarFitResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: SarFitModelIr,
    pub input: SarFitInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: SarFitPosterior,
    pub impacts: Vec<SarImpactSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: SarPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_nonstandardized_weights() {
        let mut spec = fixture();
        spec.weights[0] = 0.5;
        assert!(matches!(
            SarFitWorkerRequest::new(spec, sampling(), "lock".into(), "worker".into(), 180),
            Err(BayesError::InvalidSpec(_))
        ));
    }

    #[test]
    fn request_rejects_predictor_confounded_with_intercept() {
        let mut spec = fixture();
        spec.design.fill(1.0);
        assert!(matches!(
            SarFitWorkerRequest::new(spec, sampling(), "lock".into(), "worker".into(), 180),
            Err(BayesError::InvalidSpec(_))
        ));
    }

    fn fixture() -> SarFitSpec {
        let dimension = 6;
        let mut weights = vec![0.0; dimension * dimension];
        for row in 0..dimension {
            weights[row * dimension + (row + 1) % dimension] = 1.0;
        }
        SarFitSpec {
            model_type: SarModelType::Lag,
            region_ids: (0..dimension).map(|index| format!("r{index}")).collect(),
            weights,
            weights_digest_sha256: "a".repeat(64),
            response: vec![0.0; dimension],
            design: (0..dimension).map(|index| index as f64).collect(),
            predictor_names: vec!["x".into()],
            intercept_prior_mean: 0.0,
            intercept_prior_sd: 2.0,
            coefficient_prior_sd: 2.0,
            rho_bound: 0.95,
            sigma_prior_sd: 1.0,
        }
    }

    fn sampling() -> NutsSamplingSpec {
        NutsSamplingSpec {
            chains: 2,
            tune_per_chain: 500,
            draws_per_chain: 1_000,
            target_accept: 0.9,
            seed: 1,
        }
    }
}

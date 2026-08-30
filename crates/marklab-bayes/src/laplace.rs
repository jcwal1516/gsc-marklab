use crate::validation::all_finite as finite;

use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION, WORKER_REQUEST_FORMAT,
        WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, SarScalarSummary, WorkerBackend,
};

const LAPLACE_BACKEND_VERSION: &str = "scipy-1.18.1+pytensor-3.2.4";

#[derive(Clone, Debug, Serialize)]
pub struct PoissonExposureObservation {
    pub observation_id: String,
    pub count: u64,
    pub exposure: f64,
}

#[derive(Clone, Debug)]
pub struct PoissonLaplaceSpec {
    pub prior_mean: f64,
    pub prior_sd: f64,
    pub initial_log_rate: f64,
    pub max_iterations: u32,
    pub gradient_tolerance: f64,
    pub observations: Vec<PoissonExposureObservation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PoissonLaplaceModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub parameter: &'static str,
    pub parameter_support: &'static str,
    pub prior_family: &'static str,
    pub prior_mean: f64,
    pub prior_sd: f64,
    pub likelihood: &'static str,
    pub link: &'static str,
    pub transformation: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct LaplaceOptimizerSpec {
    pub initial_log_rate: f64,
    pub max_iterations: u32,
    pub gradient_tolerance: f64,
    pub method: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct PoissonLaplaceWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: PoissonLaplaceModelIr,
    pub observations: Vec<PoissonExposureObservation>,
    pub optimizer: LaplaceOptimizerSpec,
    pub resources: WorkerResourceLimits,
    pub seed: u64,
}

impl PoissonLaplaceWorkerRequest {
    pub fn new(
        mut spec: PoissonLaplaceSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
        seed: u64,
    ) -> Result<Self, BayesError> {
        validate_spec(&mut spec)?;
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "Laplace worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "scipy_pytensor",
                version: LAPLACE_BACKEND_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: PoissonLaplaceModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "poisson_log_rate",
                parameter: "log_rate",
                parameter_support: "real_unconstrained",
                prior_family: "normal",
                prior_mean: spec.prior_mean,
                prior_sd: spec.prior_sd,
                likelihood: "poisson_exposure",
                link: "log",
                transformation: "rate_equals_exp_log_rate",
                backend_capability: "laplace_approximation",
                maturity: "experimental_approximation",
            },
            observations: spec.observations,
            optimizer: LaplaceOptimizerSpec {
                initial_log_rate: spec.initial_log_rate,
                max_iterations: spec.max_iterations,
                gradient_tolerance: spec.gradient_tolerance,
                method: "scipy_bfgs_with_exact_pytensor_gradient_newton_refinement",
            },
            resources: WorkerResourceLimits {
                maximum_observations: 100_000,
                maximum_total_iterations: u64::from(spec.max_iterations),
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
            seed,
        })
    }
}

fn validate_spec(spec: &mut PoissonLaplaceSpec) -> Result<(), BayesError> {
    if spec.observations.is_empty() || spec.observations.len() > 100_000 {
        return Err(BayesError::InvalidSpec(
            "Poisson Laplace requires 1-100000 observations".into(),
        ));
    }
    if !spec.prior_mean.is_finite()
        || !spec.prior_sd.is_finite()
        || spec.prior_sd <= 0.0
        || !spec.initial_log_rate.is_finite()
        || !(1..=100_000).contains(&spec.max_iterations)
        || !spec.gradient_tolerance.is_finite()
        || spec.gradient_tolerance <= 0.0
        || spec.gradient_tolerance > 1e-2
    {
        return Err(BayesError::InvalidSpec(
            "Poisson Laplace prior or optimizer controls are invalid".into(),
        ));
    }
    spec.observations
        .sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    for (index, observation) in spec.observations.iter().enumerate() {
        if observation.observation_id.is_empty()
            || observation.observation_id.trim() != observation.observation_id
            || !observation.exposure.is_finite()
            || observation.exposure <= 0.0
            || (index > 0
                && spec.observations[index - 1].observation_id == observation.observation_id)
        {
            return Err(BayesError::InvalidSpec(
                "Poisson Laplace IDs must be exact/unique and exposures finite/positive".into(),
            ));
        }
    }
    let count_total = spec.observations.iter().try_fold(0_u64, |total, row| {
        (row.count <= i64::MAX as u64)
            .then(|| total.checked_add(row.count))
            .flatten()
    });
    let exposure_total = spec
        .observations
        .iter()
        .map(|row| row.exposure)
        .sum::<f64>();
    if count_total.is_none_or(|total| total > i64::MAX as u64) || !exposure_total.is_finite() {
        return Err(BayesError::InvalidSpec(
            "Poisson Laplace aggregate count or exposure exceeds the backend range".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaplaceOptimizerDiagnostics {
    pub method: String,
    pub success: bool,
    pub iterations: u32,
    pub function_evaluations: u32,
    pub gradient_evaluations: u32,
    pub gradient_norm: f64,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaplaceMode {
    pub log_rate: f64,
    pub log_joint: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaplaceHessian {
    pub negative_hessian: f64,
    pub variance: f64,
    pub standard_deviation: f64,
    pub condition_number: f64,
    pub positive_definite: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaplaceRateApproximation {
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
    pub transformation: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaplacePosteriorPredictive {
    pub observed_total_count: u64,
    pub replicated_total_mean: f64,
    pub replicated_total_sd: f64,
    pub observed_zero_count: u64,
    pub replicated_zero_count_mean: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoissonLaplaceWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub optimizer: LaplaceOptimizerDiagnostics,
    pub mode: LaplaceMode,
    pub hessian: LaplaceHessian,
    pub log_rate_approximation: SarScalarSummary,
    pub rate_approximation: LaplaceRateApproximation,
    pub posterior_predictive: LaplacePosteriorPredictive,
}

impl PoissonLaplaceWorkerResult {
    pub fn validate(
        &self,
        request: &PoissonLaplaceWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::Complete
            || self.format != "marklab.scipy_pytensor_poisson_laplace_worker_result"
            || self.version != 1
            || self.backend.name != "scipy_pytensor"
            || self.backend.version != LAPLACE_BACKEND_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "Poisson Laplace result identity mismatch".into(),
            ));
        }
        if self.optimizer.method != request.optimizer.method
            || self.optimizer.iterations > request.optimizer.max_iterations
            || !finite(&[
                self.optimizer.gradient_norm,
                self.mode.log_rate,
                self.mode.log_joint,
                self.hessian.negative_hessian,
                self.hessian.variance,
                self.hessian.standard_deviation,
                self.hessian.condition_number,
                self.log_rate_approximation.mean,
                self.log_rate_approximation.sd,
                self.log_rate_approximation.interval_lower,
                self.log_rate_approximation.interval_upper,
                self.rate_approximation.mean,
                self.rate_approximation.sd,
                self.rate_approximation.interval_lower,
                self.rate_approximation.interval_upper,
                self.posterior_predictive.replicated_total_mean,
                self.posterior_predictive.replicated_total_sd,
                self.posterior_predictive.replicated_zero_count_mean,
            ])
            || self.hessian.negative_hessian <= 0.0
            || self.hessian.variance <= 0.0
            || self.hessian.standard_deviation <= 0.0
            || self.hessian.condition_number < 1.0
            || self.log_rate_approximation.sd <= 0.0
            || self.log_rate_approximation.interval_lower
                > self.log_rate_approximation.interval_upper
            || self.rate_approximation.mean <= 0.0
            || self.rate_approximation.sd <= 0.0
            || self.rate_approximation.interval_lower <= 0.0
            || self.rate_approximation.interval_lower > self.rate_approximation.interval_upper
            || self.rate_approximation.transformation != "lognormal_from_gaussian_log_rate"
            || self.posterior_predictive.replicated_total_sd < 0.0
            || self.posterior_predictive.replicated_zero_count_mean < 0.0
        {
            return Err(BayesError::WorkerContract(
                "Poisson Laplace optimizer, Hessian, or approximation is invalid".into(),
            ));
        }
        let observed_total = request
            .observations
            .iter()
            .map(|row| row.count)
            .sum::<u64>();
        let observed_zeros = request
            .observations
            .iter()
            .filter(|row| row.count == 0)
            .count() as u64;
        if self.posterior_predictive.observed_total_count != observed_total
            || self.posterior_predictive.observed_zero_count != observed_zeros
        {
            return Err(BayesError::WorkerContract(
                "Poisson Laplace predictive changed observed summaries".into(),
            ));
        }
        let valid = self.optimizer.success
            && self.optimizer.gradient_norm <= request.optimizer.gradient_tolerance
            && self.hessian.positive_definite;
        if (self.fit_state == FitState::ApproximateOnly) != valid {
            return Err(BayesError::WorkerContract(
                "Poisson Laplace fit state disagrees with approximation diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: PoissonLaplaceWorkerRequest,
        input: PoissonLaplaceInputIdentity,
    ) -> PoissonLaplaceFit {
        PoissonLaplaceFit {
            format: "marklab.bayesian_poisson_log_rate_laplace",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            fit_state: self.fit_state,
            claim_status: match self.fit_state {
                FitState::ApproximateOnly => "experimental_approximate_only",
                FitState::Nonconverged => "diagnostic_only_nonconverged",
                FitState::Complete => "experimental",
            },
            optimizer: self.optimizer,
            mode: self.mode,
            hessian: self.hessian,
            log_rate_approximation: self.log_rate_approximation,
            rate_approximation: self.rate_approximation,
            posterior_predictive: self.posterior_predictive,
            seed: request.seed,
            request_sha256: self.request_sha256,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PoissonLaplaceInputIdentity {
    pub path: String,
    pub observation_count: usize,
    pub observations_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PoissonLaplaceFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: PoissonLaplaceModelIr,
    pub input: PoissonLaplaceInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub optimizer: LaplaceOptimizerDiagnostics,
    pub mode: LaplaceMode,
    pub hessian: LaplaceHessian,
    pub log_rate_approximation: SarScalarSummary,
    pub rate_approximation: LaplaceRateApproximation,
    pub posterior_predictive: LaplacePosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_count_total_outside_signed_backend_range() {
        let spec = PoissonLaplaceSpec {
            prior_mean: 0.0,
            prior_sd: 1.0,
            initial_log_rate: 0.0,
            max_iterations: 100,
            gradient_tolerance: 1e-8,
            observations: vec![
                PoissonExposureObservation {
                    observation_id: "a".into(),
                    count: i64::MAX as u64,
                    exposure: 1.0,
                },
                PoissonExposureObservation {
                    observation_id: "b".into(),
                    count: 1,
                    exposure: 1.0,
                },
            ],
        };
        assert!(matches!(
            PoissonLaplaceWorkerRequest::new(spec, "lock".into(), "worker".into(), 180, 1,),
            Err(BayesError::InvalidSpec(_))
        ));
    }
}

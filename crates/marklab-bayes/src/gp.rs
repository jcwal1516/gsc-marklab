use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, DiagnosticPolicy, MODEL_FORMAT, MODEL_VERSION, PYMC_VERSION,
        WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct GpObservation {
    pub observation_id: String,
    pub x_um: f64,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GpPredictionCoordinate {
    pub prediction_id: String,
    pub x_um: f64,
}

#[derive(Clone, Debug)]
pub struct ExactGpSpec {
    pub mean_prior_mean: f64,
    pub mean_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: f64,
    pub noise_prior_sd: f64,
    pub jitter: f64,
    pub observations: Vec<GpObservation>,
    pub predictions: Vec<GpPredictionCoordinate>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExactGpModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub coordinate_dimension: u32,
    pub coordinate_unit: &'static str,
    pub kernel: &'static str,
    pub mean_prior_mean: f64,
    pub mean_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: f64,
    pub noise_prior_sd: f64,
    pub jitter: f64,
    pub observation_unit: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExactGpWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: ExactGpModelIr,
    pub observations: Vec<GpObservation>,
    pub predictions: Vec<GpPredictionCoordinate>,
    pub sampling: NutsSamplingSpec,
    pub resources: GpResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl ExactGpWorkerRequest {
    pub fn new(
        mut spec: ExactGpSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if !spec.mean_prior_mean.is_finite() {
            return Err(BayesError::InvalidSpec(
                "mean prior mean must be finite".into(),
            ));
        }
        for (value, name) in [
            (spec.mean_prior_sd, "mean prior SD"),
            (spec.amplitude_prior_sd, "amplitude prior SD"),
            (spec.length_scale_prior_sd_um, "length-scale prior SD"),
            (spec.noise_prior_sd, "noise prior SD"),
            (spec.jitter, "jitter"),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(BayesError::InvalidSpec(format!(
                    "{name} must be finite and positive"
                )));
            }
        }
        if !(5..=128).contains(&spec.observations.len())
            || !(1..=2_048).contains(&spec.predictions.len())
        {
            return Err(BayesError::InvalidSpec(
                "exact GP requires 5-128 observations and 1-2048 predictions".into(),
            ));
        }
        spec.observations
            .sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
        spec.predictions
            .sort_by(|left, right| left.prediction_id.cmp(&right.prediction_id));
        let mut observation_ids = BTreeSet::new();
        let mut coordinate_bits = BTreeSet::new();
        for observation in &spec.observations {
            if observation.observation_id.is_empty()
                || observation.observation_id.trim() != observation.observation_id
                || !observation.x_um.is_finite()
                || !observation.value.is_finite()
                || !observation_ids.insert(observation.observation_id.as_str())
                || !coordinate_bits.insert(observation.x_um.to_bits())
            {
                return Err(BayesError::InvalidSpec(
                    "observation IDs/coordinates must be exact, finite, and unique".into(),
                ));
            }
        }
        let mut prediction_ids = BTreeSet::new();
        for prediction in &spec.predictions {
            if prediction.prediction_id.is_empty()
                || prediction.prediction_id.trim() != prediction.prediction_id
                || !prediction.x_um.is_finite()
                || !prediction_ids.insert(prediction.prediction_id.as_str())
            {
                return Err(BayesError::InvalidSpec(
                    "prediction IDs must be exact and unique with finite coordinates".into(),
                ));
            }
        }
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let maximum_total_iterations = 800_000;
        let total_iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        if total_iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "requested NUTS iterations exceed 800000".into(),
            ));
        }
        let n = spec.observations.len() as u128;
        let m = spec.predictions.len() as u128;
        let posterior_draws = u128::from(sampling.chains) * u128::from(sampling.draws_per_chain);
        let work = u128::from(total_iterations) * n.pow(3) + posterior_draws * n.pow(2) * m;
        let maximum_conditioning_work = 2_000_000_000_u64;
        if work > u128::from(maximum_conditioning_work) {
            return Err(BayesError::InvalidSpec(format!(
                "requested exact GP work {work} exceeds {maximum_conditioning_work}"
            )));
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
            model: ExactGpModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "exact_matern32_gp_regression",
                coordinate_dimension: 1,
                coordinate_unit: "micrometre",
                kernel: "matern_3_2",
                mean_prior_mean: spec.mean_prior_mean,
                mean_prior_sd: spec.mean_prior_sd,
                amplitude_prior_sd: spec.amplitude_prior_sd,
                length_scale_prior_sd_um: spec.length_scale_prior_sd_um,
                noise_prior_sd: spec.noise_prior_sd,
                jitter: spec.jitter,
                observation_unit: "scalar_field_value",
                backend_capability: "nuts_exact_dense_gp",
                maturity: "experimental",
            },
            observations: spec.observations,
            predictions: spec.predictions,
            sampling,
            resources: GpResourceLimits {
                maximum_observations: 128,
                maximum_predictions: 2_048,
                maximum_total_iterations,
                maximum_conditioning_work,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GpResourceLimits {
    pub maximum_observations: u32,
    pub maximum_predictions: u32,
    pub maximum_total_iterations: u64,
    pub maximum_conditioning_work: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GpScalarSummary {
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GpPosterior {
    pub mean: GpScalarSummary,
    pub amplitude: GpScalarSummary,
    pub length_scale_um: GpScalarSummary,
    pub noise_sd: GpScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GpPredictionSummary {
    pub prediction_id: String,
    pub x_um: f64,
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GpPosteriorPredictive {
    pub observed_mean: f64,
    pub replicated_mean: f64,
    pub replicated_mean_sd: f64,
    pub observed_sd: f64,
    pub replicated_sd_mean: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactGpWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GpPosterior,
    pub predictions: Vec<GpPredictionSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: GpPosteriorPredictive,
}

impl ExactGpWorkerResult {
    pub fn validate(
        &self,
        request: &ExactGpWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly {
            return Err(BayesError::WorkerContract(
                "exact GP NUTS cannot return approximate-only state".into(),
            ));
        }
        if self.format != "marklab.pymc_gp_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.predictions.len() != request.predictions.len()
        {
            return Err(BayesError::WorkerContract(
                "GP result identity mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
        {
            return Err(BayesError::WorkerContract(
                "GP sampling counts mismatch".into(),
            ));
        }
        for summary in [
            &self.posterior.mean,
            &self.posterior.amplitude,
            &self.posterior.length_scale_um,
            &self.posterior.noise_sd,
        ] {
            validate_scalar(summary)?;
        }
        if self.posterior.amplitude.mean <= 0.0
            || self.posterior.amplitude.interval_lower < 0.0
            || self.posterior.length_scale_um.mean <= 0.0
            || self.posterior.length_scale_um.interval_lower < 0.0
            || self.posterior.noise_sd.mean <= 0.0
            || self.posterior.noise_sd.interval_lower < 0.0
        {
            return Err(BayesError::WorkerContract(
                "invalid GP hyperparameters".into(),
            ));
        }
        for (actual, expected) in self.predictions.iter().zip(&request.predictions) {
            if actual.prediction_id != expected.prediction_id
                || actual.x_um.to_bits() != expected.x_um.to_bits()
                || !finite(&[
                    actual.mean,
                    actual.sd,
                    actual.interval_lower,
                    actual.interval_upper,
                ])
                || actual.sd <= 0.0
                || actual.interval_lower > actual.interval_upper
            {
                return Err(BayesError::WorkerContract("invalid GP prediction".into()));
            }
        }
        if !finite(&[
            self.diagnostics.r_hat,
            self.diagnostics.ess_bulk,
            self.diagnostics.ess_tail,
            self.diagnostics.mcse_mean,
            self.diagnostics.mcse_sd,
            self.diagnostics.minimum_ebfmi,
            self.posterior_predictive.observed_mean,
            self.posterior_predictive.replicated_mean,
            self.posterior_predictive.replicated_mean_sd,
            self.posterior_predictive.observed_sd,
            self.posterior_predictive.replicated_sd_mean,
        ]) || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
            || self.posterior_predictive.replicated_mean_sd < 0.0
            || self.posterior_predictive.observed_sd < 0.0
            || self.posterior_predictive.replicated_sd_mean < 0.0
        {
            return Err(BayesError::WorkerContract("invalid GP diagnostics".into()));
        }
        let observed_mean = request
            .observations
            .iter()
            .map(|observation| observation.value)
            .sum::<f64>()
            / request.observations.len() as f64;
        let observed_sd = (request
            .observations
            .iter()
            .map(|observation| (observation.value - observed_mean).powi(2))
            .sum::<f64>()
            / (request.observations.len() - 1) as f64)
            .sqrt();
        if (self.posterior_predictive.observed_mean - observed_mean).abs()
            > 1e-12 * observed_mean.abs().max(1.0)
            || (self.posterior_predictive.observed_sd - observed_sd).abs()
                > 1e-12 * observed_sd.abs().max(1.0)
        {
            return Err(BayesError::WorkerContract(
                "GP posterior predictive changed observed summaries".into(),
            ));
        }
        let complete = self.diagnostics.prior_predictive_finite
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
        if (self.fit_state == FitState::Complete) != complete {
            return Err(BayesError::WorkerContract(
                "GP fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: ExactGpWorkerRequest,
        input: ExactGpInputIdentity,
    ) -> ExactGpFit {
        ExactGpFit {
            format: "marklab.bayesian_gp_fit",
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
            predictions: self.predictions,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_scalar(value: &GpScalarSummary) -> Result<(), BayesError> {
    if !finite(&[
        value.mean,
        value.sd,
        value.interval_lower,
        value.interval_upper,
    ]) || value.sd <= 0.0
        || value.interval_lower > value.interval_upper
    {
        return Err(BayesError::WorkerContract(
            "invalid GP scalar summary".into(),
        ));
    }
    Ok(())
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[derive(Debug, Serialize)]
pub struct ExactGpInputIdentity {
    pub observation_path: String,
    pub prediction_path: String,
    pub observations: usize,
    pub predictions: usize,
    pub data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct ExactGpFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: ExactGpModelIr,
    pub input: ExactGpInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: GpPosterior,
    pub predictions: Vec<GpPredictionSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: GpPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_gp_rejects_duplicate_observation_coordinate() {
        let result = ExactGpWorkerRequest::new(
            ExactGpSpec {
                mean_prior_mean: 0.0,
                mean_prior_sd: 5.0,
                amplitude_prior_sd: 2.0,
                length_scale_prior_sd_um: 5.0,
                noise_prior_sd: 1.0,
                jitter: 1e-6,
                observations: vec![
                    observation("o-1", 0.0),
                    observation("o-2", 1.0),
                    observation("o-3", 2.0),
                    observation("o-4", 3.0),
                    observation("o-5", 3.0),
                ],
                predictions: vec![GpPredictionCoordinate {
                    prediction_id: "p-1".into(),
                    x_um: 0.5,
                }],
            },
            NutsSamplingSpec {
                chains: 2,
                tune_per_chain: 500,
                draws_per_chain: 1_000,
                target_accept: 0.9,
                seed: 1,
            },
            "lock".into(),
            "worker".into(),
            180,
        );
        assert!(matches!(result, Err(BayesError::InvalidSpec(_))));
    }

    fn observation(observation_id: &str, x_um: f64) -> GpObservation {
        GpObservation {
            observation_id: observation_id.into(),
            x_um,
            value: x_um,
        }
    }
}

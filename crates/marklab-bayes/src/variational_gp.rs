use crate::validation::all_finite as finite;

use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, MODEL_FORMAT, MODEL_VERSION, PYMC_VERSION, WORKER_REQUEST_FORMAT,
        WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, GpObservation, GpPredictionCoordinate, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct VariationalGpSpec {
    pub mean_prior_mean: f64,
    pub mean_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: f64,
    pub noise_prior_sd: f64,
    pub jitter: f64,
    pub inducing_points: u32,
    pub starts: u32,
    pub iterations: u32,
    pub learning_rate: f64,
    pub posterior_draws: u32,
    pub seed: u64,
    pub observations: Vec<GpObservation>,
    pub predictions: Vec<GpPredictionCoordinate>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariationalGpModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub approximation: &'static str,
    pub coordinate_dimension: u32,
    pub coordinate_unit: &'static str,
    pub kernel: &'static str,
    pub mean_prior_mean: f64,
    pub mean_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: f64,
    pub noise_prior_sd: f64,
    pub jitter: f64,
    pub inducing_initialization: &'static str,
    pub inducing_constraint: &'static str,
    pub fit_state_ceiling: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariationalInferenceSpec {
    pub inducing_points: u32,
    pub starts: u32,
    pub iterations: u32,
    pub learning_rate: f64,
    pub posterior_draws: u32,
    pub maximum_cross_start_prediction_rmse: f64,
    pub maximum_tail_relative_change: f64,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariationalResourceLimits {
    pub maximum_observations: u32,
    pub maximum_predictions: u32,
    pub maximum_inducing_points: u32,
    pub maximum_variational_work: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariationalGpWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: VariationalGpModelIr,
    pub observations: Vec<GpObservation>,
    pub predictions: Vec<GpPredictionCoordinate>,
    pub inference: VariationalInferenceSpec,
    pub resources: VariationalResourceLimits,
}

impl VariationalGpWorkerRequest {
    pub fn new(
        mut spec: VariationalGpSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        for (value, name) in [
            (spec.mean_prior_mean, "mean prior mean"),
            (spec.mean_prior_sd, "mean prior SD"),
            (spec.amplitude_prior_sd, "amplitude prior SD"),
            (spec.length_scale_prior_sd_um, "length prior SD"),
            (spec.noise_prior_sd, "noise prior SD"),
            (spec.jitter, "jitter"),
            (spec.learning_rate, "learning rate"),
        ] {
            if !value.is_finite() || (name != "mean prior mean" && value <= 0.0) {
                return Err(BayesError::InvalidSpec(format!("{name} is invalid")));
            }
        }
        if !(8..=2_000).contains(&spec.observations.len())
            || !(1..=2_048).contains(&spec.predictions.len())
            || !(3..=64).contains(&spec.inducing_points)
            || spec.inducing_points as usize >= spec.observations.len()
            || !(2..=4).contains(&spec.starts)
            || !(1_000..=100_000).contains(&spec.iterations)
            || !(500..=10_000).contains(&spec.posterior_draws)
            || spec.learning_rate > 0.1
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "variational GP counts, learning rate, or timeout are outside limits".into(),
            ));
        }
        spec.observations
            .sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
        spec.predictions
            .sort_by(|left, right| left.prediction_id.cmp(&right.prediction_id));
        let mut ids = HashSet::new();
        let mut coordinates = BTreeSet::new();
        for row in &spec.observations {
            if row.observation_id.is_empty()
                || row.observation_id.trim() != row.observation_id
                || !row.x_um.is_finite()
                || !row.value.is_finite()
                || !ids.insert(row.observation_id.as_str())
                || !coordinates.insert(row.x_um.to_bits())
            {
                return Err(BayesError::InvalidSpec(
                    "variational GP observations require exact unique IDs/coordinates and finite values".into(),
                ));
            }
        }
        let mut prediction_ids = HashSet::new();
        for row in &spec.predictions {
            if row.prediction_id.is_empty()
                || row.prediction_id.trim() != row.prediction_id
                || !row.x_um.is_finite()
                || !prediction_ids.insert(row.prediction_id.as_str())
            {
                return Err(BayesError::InvalidSpec(
                    "variational GP predictions require exact unique IDs and finite coordinates"
                        .into(),
                ));
            }
        }
        let n = spec.observations.len() as u128;
        let m = u128::from(spec.inducing_points);
        let p = spec.predictions.len() as u128;
        let work =
            u128::from(spec.starts) * u128::from(spec.iterations) * (n * m.pow(2) + m.pow(3))
                + u128::from(spec.starts) * u128::from(spec.posterior_draws) * p * m.pow(2);
        let maximum_variational_work = 5_000_000_000_u64;
        if work > u128::from(maximum_variational_work) {
            return Err(BayesError::InvalidSpec(format!(
                "requested variational work {work} exceeds {maximum_variational_work}"
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
            model: VariationalGpModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "variational_inducing_point_matern32_gp",
                approximation: "vfe_inducing_point_mean_field_advi",
                coordinate_dimension: 1,
                coordinate_unit: "micrometre",
                kernel: "matern_3_2",
                mean_prior_mean: spec.mean_prior_mean,
                mean_prior_sd: spec.mean_prior_sd,
                amplitude_prior_sd: spec.amplitude_prior_sd,
                length_scale_prior_sd_um: spec.length_scale_prior_sd_um,
                noise_prior_sd: spec.noise_prior_sd,
                jitter: spec.jitter,
                inducing_initialization: "coordinate_quantiles",
                inducing_constraint: "strictly_ordered",
                fit_state_ceiling: "approximate_only",
                maturity: "experimental",
            },
            observations: spec.observations,
            predictions: spec.predictions,
            inference: VariationalInferenceSpec {
                inducing_points: spec.inducing_points,
                starts: spec.starts,
                iterations: spec.iterations,
                learning_rate: spec.learning_rate,
                posterior_draws: spec.posterior_draws,
                maximum_cross_start_prediction_rmse: 0.5,
                maximum_tail_relative_change: 0.2,
                seed: spec.seed,
            },
            resources: VariationalResourceLimits {
                maximum_observations: 2_000,
                maximum_predictions: 2_048,
                maximum_inducing_points: 64,
                maximum_variational_work,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariationalScalarSummary {
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariationalPosterior {
    pub mean: VariationalScalarSummary,
    pub amplitude: VariationalScalarSummary,
    pub length_scale_um: VariationalScalarSummary,
    pub noise_sd: VariationalScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariationalStartDiagnostic {
    pub start_index: u32,
    pub initial_elbo: f64,
    pub final_elbo: f64,
    pub tail_relative_change: f64,
    pub finite: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariationalDiagnostics {
    pub starts: Vec<VariationalStartDiagnostic>,
    pub selected_start: u32,
    pub all_finite: bool,
    pub prior_predictive_finite: bool,
    pub posterior_finite: bool,
    pub cross_start_prediction_rmse: f64,
    pub gradient_norm_available: bool,
    pub importance_correction: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariationalPosteriorPredictive {
    pub observed_mean: f64,
    pub replicated_mean: f64,
    pub observed_sd: f64,
    pub replicated_sd_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariationalPrediction {
    pub prediction_id: String,
    pub x_um: f64,
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariationalGpWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub posterior: VariationalPosterior,
    pub inducing_locations_um: Vec<f64>,
    pub predictions: Vec<VariationalPrediction>,
    pub diagnostics: VariationalDiagnostics,
    pub posterior_predictive: VariationalPosteriorPredictive,
}

impl VariationalGpWorkerResult {
    pub fn validate(
        &self,
        request: &VariationalGpWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.pymc_variational_gp_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.fit_state == FitState::Complete
            || self.inducing_locations_um.len() != request.inference.inducing_points as usize
            || self.predictions.len() != request.predictions.len()
            || self.diagnostics.starts.len() != request.inference.starts as usize
        {
            return Err(BayesError::WorkerContract(
                "variational GP result identity/count/state mismatch".into(),
            ));
        }
        for summary in [
            &self.posterior.mean,
            &self.posterior.amplitude,
            &self.posterior.length_scale_um,
            &self.posterior.noise_sd,
        ] {
            if !finite(&[
                summary.mean,
                summary.sd,
                summary.interval_lower,
                summary.interval_upper,
            ]) || summary.sd <= 0.0
                || summary.interval_lower > summary.interval_upper
            {
                return Err(BayesError::WorkerContract(
                    "invalid variational hyperparameter summary".into(),
                ));
            }
        }
        if !self
            .inducing_locations_um
            .windows(2)
            .all(|pair| pair[0].is_finite() && pair[0] < pair[1])
            || !self
                .inducing_locations_um
                .last()
                .is_some_and(|value| value.is_finite())
            || !self.diagnostics.all_finite
            || !self.diagnostics.prior_predictive_finite
            || !self.diagnostics.posterior_finite
            || !self.diagnostics.cross_start_prediction_rmse.is_finite()
            || self.diagnostics.cross_start_prediction_rmse < 0.0
            || self.diagnostics.selected_start >= request.inference.starts
            || self.diagnostics.gradient_norm_available
            || self.diagnostics.importance_correction != "not_run"
        {
            return Err(BayesError::WorkerContract(
                "invalid variational diagnostics or inducing locations".into(),
            ));
        }
        if !finite(&[
            self.posterior_predictive.observed_mean,
            self.posterior_predictive.replicated_mean,
            self.posterior_predictive.observed_sd,
            self.posterior_predictive.replicated_sd_mean,
        ]) || self.posterior_predictive.observed_sd < 0.0
            || self.posterior_predictive.replicated_sd_mean < 0.0
        {
            return Err(BayesError::WorkerContract(
                "invalid variational posterior predictive result".into(),
            ));
        }
        for (index, start) in self.diagnostics.starts.iter().enumerate() {
            if start.start_index != index as u32
                || !start.finite
                || !finite(&[
                    start.initial_elbo,
                    start.final_elbo,
                    start.tail_relative_change,
                ])
                || start.tail_relative_change < 0.0
            {
                return Err(BayesError::WorkerContract(
                    "invalid variational start diagnostics".into(),
                ));
            }
        }
        let approximation_pass = self.diagnostics.all_finite
            && self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.cross_start_prediction_rmse
                <= request.inference.maximum_cross_start_prediction_rmse
            && self.diagnostics.starts.iter().all(|start| {
                start.final_elbo > start.initial_elbo
                    && start.tail_relative_change <= request.inference.maximum_tail_relative_change
            });
        if (self.fit_state == FitState::ApproximateOnly) != approximation_pass {
            return Err(BayesError::WorkerContract(
                "variational fit state disagrees with approximation diagnostics".into(),
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
                return Err(BayesError::WorkerContract(
                    "invalid variational prediction".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: VariationalGpWorkerRequest,
        input: VariationalGpInputIdentity,
    ) -> VariationalGpFit {
        VariationalGpFit {
            format: "marklab.bayesian_variational_gp_fit",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            inference: request.inference,
            fit_state: self.fit_state,
            claim_status: match self.fit_state {
                FitState::ApproximateOnly => "experimental_approximate_only",
                FitState::Nonconverged => "diagnostic_only_nonconverged",
                FitState::Complete => "invalid_complete_state",
            },
            posterior: self.posterior,
            inducing_locations_um: self.inducing_locations_um,
            predictions: self.predictions,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            request_sha256: self.request_sha256,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VariationalGpInputIdentity {
    pub observation_path: String,
    pub prediction_path: String,
    pub observations: usize,
    pub predictions: usize,
    pub data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct VariationalGpFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: VariationalGpModelIr,
    pub input: VariationalGpInputIdentity,
    pub inference: VariationalInferenceSpec,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub posterior: VariationalPosterior,
    pub inducing_locations_um: Vec<f64>,
    pub predictions: Vec<VariationalPrediction>,
    pub diagnostics: VariationalDiagnostics,
    pub posterior_predictive: VariationalPosteriorPredictive,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variational_gp_requires_fewer_inducing_points_than_observations() {
        let observations = (0..8)
            .map(|index| GpObservation {
                observation_id: format!("o-{index}"),
                x_um: index as f64,
                value: index as f64,
            })
            .collect();
        let result = VariationalGpWorkerRequest::new(
            VariationalGpSpec {
                mean_prior_mean: 0.0,
                mean_prior_sd: 5.0,
                amplitude_prior_sd: 2.0,
                length_scale_prior_sd_um: 5.0,
                noise_prior_sd: 1.0,
                jitter: 1e-6,
                inducing_points: 8,
                starts: 2,
                iterations: 1_000,
                learning_rate: 0.01,
                posterior_draws: 500,
                seed: 1,
                observations,
                predictions: vec![GpPredictionCoordinate {
                    prediction_id: "p-1".into(),
                    x_um: 0.5,
                }],
            },
            "lock".into(),
            "worker".into(),
            180,
        );
        assert!(matches!(result, Err(BayesError::InvalidSpec(_))));
    }
}

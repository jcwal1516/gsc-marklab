use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    gp::{GpPosteriorPredictive, GpScalarSummary},
    model::{
        BackendContract, DiagnosticPolicy, MODEL_FORMAT, MODEL_VERSION, PYMC_VERSION,
        WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct AnisotropicGp3dObservation {
    pub observation_id: String,
    pub coordinates_um: [f64; 3],
    pub value: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AnisotropicGp3dPredictionCoordinate {
    pub prediction_id: String,
    pub coordinates_um: [f64; 3],
}

#[derive(Clone, Debug)]
pub struct AnisotropicGp3dSpec {
    pub mean_prior_mean: f64,
    pub mean_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: [f64; 3],
    pub noise_prior_sd: f64,
    pub jitter: f64,
    pub observations: Vec<AnisotropicGp3dObservation>,
    pub predictions: Vec<AnisotropicGp3dPredictionCoordinate>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AnisotropicGp3dModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub coordinate_dimension: u32,
    pub coordinate_unit: &'static str,
    pub kernel: &'static str,
    pub anisotropy: &'static str,
    pub mean_prior_mean: f64,
    pub mean_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: [f64; 3],
    pub noise_prior_sd: f64,
    pub jitter: f64,
    pub observation_unit: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct AnisotropicGp3dWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: AnisotropicGp3dModelIr,
    pub observations: Vec<AnisotropicGp3dObservation>,
    pub predictions: Vec<AnisotropicGp3dPredictionCoordinate>,
    pub sampling: NutsSamplingSpec,
    pub resources: AnisotropicGp3dResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl AnisotropicGp3dWorkerRequest {
    pub fn new(
        mut spec: AnisotropicGp3dSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if !spec.mean_prior_mean.is_finite() {
            return Err(BayesError::InvalidSpec(
                "3-D GP mean prior mean must be finite".into(),
            ));
        }
        for (value, name) in [
            (spec.mean_prior_sd, "mean prior SD"),
            (spec.amplitude_prior_sd, "amplitude prior SD"),
            (spec.length_scale_prior_sd_um[0], "x length-scale prior SD"),
            (spec.length_scale_prior_sd_um[1], "y length-scale prior SD"),
            (spec.length_scale_prior_sd_um[2], "z length-scale prior SD"),
            (spec.noise_prior_sd, "noise prior SD"),
            (spec.jitter, "jitter"),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(BayesError::InvalidSpec(format!(
                    "3-D GP {name} must be finite and positive"
                )));
            }
        }
        if !(8..=64).contains(&spec.observations.len())
            || !(1..=512).contains(&spec.predictions.len())
        {
            return Err(BayesError::InvalidSpec(
                "anisotropic 3-D GP requires 8-64 observations and 1-512 predictions".into(),
            ));
        }
        spec.observations
            .sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
        spec.predictions
            .sort_by(|left, right| left.prediction_id.cmp(&right.prediction_id));
        let mut observation_ids = BTreeSet::new();
        let mut coordinate_bits = BTreeSet::new();
        let mut distinct_by_axis = [BTreeSet::new(), BTreeSet::new(), BTreeSet::new()];
        for observation in &spec.observations {
            if observation.observation_id.is_empty()
                || observation.observation_id.trim() != observation.observation_id
                || observation
                    .coordinates_um
                    .iter()
                    .any(|value| !value.is_finite())
                || !observation.value.is_finite()
                || !observation_ids.insert(observation.observation_id.as_str())
                || !coordinate_bits.insert(observation.coordinates_um.map(f64::to_bits))
            {
                return Err(BayesError::InvalidSpec(
                    "3-D GP observation IDs/coordinates must be exact, finite, and unique".into(),
                ));
            }
            for (axis, coordinate) in observation.coordinates_um.iter().enumerate() {
                distinct_by_axis[axis].insert(coordinate.to_bits());
            }
        }
        if distinct_by_axis.iter().any(|values| values.len() < 2) {
            return Err(BayesError::InvalidSpec(
                "anisotropic 3-D GP requires variation on every coordinate axis".into(),
            ));
        }
        let mut prediction_ids = BTreeSet::new();
        for prediction in &spec.predictions {
            if prediction.prediction_id.is_empty()
                || prediction.prediction_id.trim() != prediction.prediction_id
                || prediction
                    .coordinates_um
                    .iter()
                    .any(|value| !value.is_finite())
                || !prediction_ids.insert(prediction.prediction_id.as_str())
            {
                return Err(BayesError::InvalidSpec(
                    "3-D GP prediction IDs must be exact and unique with finite coordinates".into(),
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
        let n = spec.observations.len() as u128;
        let m = spec.predictions.len() as u128;
        let posterior_draws = u128::from(sampling.chains) * u128::from(sampling.draws_per_chain);
        let work = u128::from(total_iterations) * n.pow(3) + posterior_draws * n.pow(2) * m;
        let maximum_conditioning_work = 2_000_000_000_u64;
        if total_iterations > maximum_total_iterations
            || work > u128::from(maximum_conditioning_work)
        {
            return Err(BayesError::InvalidSpec(format!(
                "requested anisotropic 3-D GP work {work} exceeds {maximum_conditioning_work}"
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
            model: AnisotropicGp3dModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "exact_axis_aligned_anisotropic_matern32_gp3d",
                coordinate_dimension: 3,
                coordinate_unit: "micrometre",
                kernel: "matern_3_2",
                anisotropy: "axis_aligned_positive_length_scales",
                mean_prior_mean: spec.mean_prior_mean,
                mean_prior_sd: spec.mean_prior_sd,
                amplitude_prior_sd: spec.amplitude_prior_sd,
                length_scale_prior_sd_um: spec.length_scale_prior_sd_um,
                noise_prior_sd: spec.noise_prior_sd,
                jitter: spec.jitter,
                observation_unit: "scalar_field_value",
                backend_capability: "nuts_exact_dense_anisotropic_gp3d",
                maturity: "experimental",
            },
            observations: spec.observations,
            predictions: spec.predictions,
            sampling,
            resources: AnisotropicGp3dResourceLimits {
                maximum_observations: 64,
                maximum_predictions: 512,
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
pub struct AnisotropicGp3dResourceLimits {
    pub maximum_observations: u32,
    pub maximum_predictions: u32,
    pub maximum_total_iterations: u64,
    pub maximum_conditioning_work: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnisotropicGp3dPosterior {
    pub mean: GpScalarSummary,
    pub amplitude: GpScalarSummary,
    pub length_scale_x_um: GpScalarSummary,
    pub length_scale_y_um: GpScalarSummary,
    pub length_scale_z_um: GpScalarSummary,
    pub noise_sd: GpScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnisotropicGp3dPredictionSummary {
    pub prediction_id: String,
    pub coordinates_um: [f64; 3],
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnisotropicGp3dWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: AnisotropicGp3dPosterior,
    pub predictions: Vec<AnisotropicGp3dPredictionSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: GpPosteriorPredictive,
}

impl AnisotropicGp3dWorkerResult {
    pub fn validate(
        &self,
        request: &AnisotropicGp3dWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.pymc_anisotropic_gp3d_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.fit_state == FitState::ApproximateOnly
            || self.predictions.len() != request.predictions.len()
        {
            return Err(BayesError::WorkerContract(
                "anisotropic 3-D GP result identity mismatch".into(),
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
                "anisotropic 3-D GP sampling counts mismatch".into(),
            ));
        }
        for summary in self.posterior_summaries() {
            validate_scalar(summary)?;
        }
        for summary in [
            &self.posterior.amplitude,
            &self.posterior.length_scale_x_um,
            &self.posterior.length_scale_y_um,
            &self.posterior.length_scale_z_um,
            &self.posterior.noise_sd,
        ] {
            if summary.mean <= 0.0 || summary.interval_lower < 0.0 {
                return Err(BayesError::WorkerContract(
                    "anisotropic 3-D GP positive parameter is invalid".into(),
                ));
            }
        }
        for (actual, expected) in self.predictions.iter().zip(&request.predictions) {
            if actual.prediction_id != expected.prediction_id
                || actual
                    .coordinates_um
                    .iter()
                    .zip(expected.coordinates_um)
                    .any(|(left, right)| left.to_bits() != right.to_bits())
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
                    "invalid anisotropic 3-D GP prediction".into(),
                ));
            }
        }
        validate_diagnostics(self, request)?;
        Ok(())
    }

    fn posterior_summaries(&self) -> [&GpScalarSummary; 6] {
        [
            &self.posterior.mean,
            &self.posterior.amplitude,
            &self.posterior.length_scale_x_um,
            &self.posterior.length_scale_y_um,
            &self.posterior.length_scale_z_um,
            &self.posterior.noise_sd,
        ]
    }

    pub fn into_fit(
        self,
        request: AnisotropicGp3dWorkerRequest,
        input: AnisotropicGp3dInputIdentity,
    ) -> AnisotropicGp3dFit {
        let lengths = [
            self.posterior.length_scale_x_um.mean,
            self.posterior.length_scale_y_um.mean,
            self.posterior.length_scale_z_um.mean,
        ];
        let metric = [
            [1.0 / lengths[0].powi(2), 0.0, 0.0],
            [0.0, 1.0 / lengths[1].powi(2), 0.0],
            [0.0, 0.0, 1.0 / lengths[2].powi(2)],
        ];
        AnisotropicGp3dFit {
            format: "marklab.bayesian_anisotropic_gp3d_fit",
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
            posterior_mean_anisotropy_metric: metric,
            predictions: self.predictions,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_diagnostics(
    result: &AnisotropicGp3dWorkerResult,
    request: &AnisotropicGp3dWorkerRequest,
) -> Result<(), BayesError> {
    let diagnostics = &result.diagnostics;
    if !finite(&[
        diagnostics.r_hat,
        diagnostics.ess_bulk,
        diagnostics.ess_tail,
        diagnostics.mcse_mean,
        diagnostics.mcse_sd,
        diagnostics.minimum_ebfmi,
        result.posterior_predictive.observed_mean,
        result.posterior_predictive.replicated_mean,
        result.posterior_predictive.replicated_mean_sd,
        result.posterior_predictive.observed_sd,
        result.posterior_predictive.replicated_sd_mean,
    ]) {
        return Err(BayesError::WorkerContract(
            "non-finite anisotropic 3-D GP diagnostics".into(),
        ));
    }
    let complete = diagnostics.prior_predictive_finite
        && diagnostics.posterior_finite
        && diagnostics.constraints_valid
        && diagnostics.identifiability_checks_passed
        && diagnostics.r_hat <= request.diagnostic_policy.maximum_r_hat
        && diagnostics.ess_bulk >= request.diagnostic_policy.minimum_bulk_ess
        && diagnostics.ess_tail >= request.diagnostic_policy.minimum_tail_ess
        && diagnostics.minimum_ebfmi >= request.diagnostic_policy.minimum_ebfmi
        && diagnostics.divergences <= request.diagnostic_policy.maximum_divergences
        && diagnostics.max_tree_depth_hits <= request.diagnostic_policy.maximum_tree_depth_hits;
    if (result.fit_state == FitState::Complete) != complete {
        return Err(BayesError::WorkerContract(
            "anisotropic 3-D GP fit state disagrees with diagnostics".into(),
        ));
    }
    Ok(())
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
            "invalid anisotropic 3-D GP scalar summary".into(),
        ));
    }
    Ok(())
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

pub fn anisotropic_matern32_covariance(
    left: [f64; 3],
    right: [f64; 3],
    amplitude: f64,
    length_scales_um: [f64; 3],
) -> Result<f64, BayesError> {
    if left.iter().chain(&right).any(|value| !value.is_finite())
        || !amplitude.is_finite()
        || amplitude <= 0.0
        || length_scales_um
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(BayesError::InvalidSpec(
            "anisotropic Matérn inputs must be finite with positive scales".into(),
        ));
    }
    let radius = left
        .iter()
        .zip(right)
        .zip(length_scales_um)
        .map(|((left, right), scale)| ((left - right) / scale).powi(2))
        .sum::<f64>()
        .sqrt();
    let scaled = 3.0_f64.sqrt() * radius;
    let covariance = amplitude.powi(2) * (1.0 + scaled) * (-scaled).exp();
    if !covariance.is_finite() {
        return Err(BayesError::InvalidSpec(
            "anisotropic Matérn covariance is not finite".into(),
        ));
    }
    Ok(covariance)
}

#[derive(Debug, Serialize)]
pub struct AnisotropicGp3dInputIdentity {
    pub observation_path: String,
    pub prediction_path: String,
    pub observations: usize,
    pub predictions: usize,
    pub data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct AnisotropicGp3dFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: AnisotropicGp3dModelIr,
    pub input: AnisotropicGp3dInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: AnisotropicGp3dPosterior,
    pub posterior_mean_anisotropy_metric: [[f64; 3]; 3],
    pub predictions: Vec<AnisotropicGp3dPredictionSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: GpPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anisotropic_matern_has_exact_zero_and_one_scaled_distance_oracles() {
        let at_zero =
            anisotropic_matern32_covariance([1.0, 2.0, 3.0], [1.0, 2.0, 3.0], 2.0, [2.0, 4.0, 8.0])
                .expect("zero distance");
        assert_eq!(at_zero, 4.0);
        let at_one =
            anisotropic_matern32_covariance([0.0, 0.0, 0.0], [2.0, 0.0, 0.0], 2.0, [2.0, 4.0, 8.0])
                .expect("one scaled distance");
        let expected = 4.0 * (1.0 + 3.0_f64.sqrt()) * (-3.0_f64.sqrt()).exp();
        assert!((at_one - expected).abs() <= f64::EPSILON * 8.0);
    }

    #[test]
    fn anisotropic_gp_requires_variation_on_every_axis() {
        let observations = (0..8)
            .map(|index| AnisotropicGp3dObservation {
                observation_id: format!("o-{index}"),
                coordinates_um: [index as f64, (index % 2) as f64, 0.0],
                value: index as f64,
            })
            .collect();
        let result = AnisotropicGp3dWorkerRequest::new(
            AnisotropicGp3dSpec {
                mean_prior_mean: 0.0,
                mean_prior_sd: 1.0,
                amplitude_prior_sd: 1.0,
                length_scale_prior_sd_um: [1.0; 3],
                noise_prior_sd: 1.0,
                jitter: 1e-6,
                observations,
                predictions: vec![AnisotropicGp3dPredictionCoordinate {
                    prediction_id: "p".into(),
                    coordinates_um: [0.0; 3],
                }],
            },
            NutsSamplingSpec {
                chains: 2,
                tune_per_chain: 100,
                draws_per_chain: 100,
                target_accept: 0.9,
                seed: 1,
            },
            "lock".into(),
            "worker".into(),
            60,
        );
        assert!(matches!(result, Err(BayesError::InvalidSpec(_))));
    }
}

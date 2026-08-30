use crate::validation::all_finite as finite;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, DiagnosticPolicy, MODEL_FORMAT, MODEL_VERSION, PYMC_VERSION,
        WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, GpPredictionCoordinate, GpResourceLimits, NormalMeanDiagnostics,
    NutsSamplingSpec, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct MultiOutputGpObservation {
    pub coordinate_id: String,
    pub x_um: f64,
    pub output_a: f64,
    pub output_b: f64,
}

#[derive(Clone, Debug)]
pub struct MultiOutputGpSpec {
    pub output_a_name: String,
    pub output_b_name: String,
    pub mean_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: f64,
    pub loading_b_prior_sd: f64,
    pub noise_a_sd: f64,
    pub noise_b_sd: f64,
    pub jitter: f64,
    pub observations: Vec<MultiOutputGpObservation>,
    pub predictions: Vec<GpPredictionCoordinate>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MultiOutputGpModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub coordinate_dimension: u32,
    pub coordinate_unit: &'static str,
    pub output_a_name: String,
    pub output_b_name: String,
    pub latent_processes: u32,
    pub kernel: &'static str,
    pub loading_a: f64,
    pub loading_b_constraint: &'static str,
    pub mean_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: f64,
    pub loading_b_prior_sd: f64,
    pub noise_a_sd: f64,
    pub noise_b_sd: f64,
    pub jitter: f64,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MultiOutputGpWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: MultiOutputGpModelIr,
    pub observations: Vec<MultiOutputGpObservation>,
    pub predictions: Vec<GpPredictionCoordinate>,
    pub sampling: NutsSamplingSpec,
    pub resources: GpResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl MultiOutputGpWorkerRequest {
    pub fn new(
        mut spec: MultiOutputGpSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        for (name, label) in [
            (&spec.output_a_name, "output A name"),
            (&spec.output_b_name, "output B name"),
        ] {
            if name.is_empty() || name.trim() != name {
                return Err(BayesError::InvalidSpec(format!(
                    "{label} must be exact and non-empty"
                )));
            }
        }
        if spec.output_a_name == spec.output_b_name {
            return Err(BayesError::InvalidSpec(
                "multi-output names must differ".into(),
            ));
        }
        for (value, name) in [
            (spec.mean_prior_sd, "mean prior SD"),
            (spec.amplitude_prior_sd, "amplitude prior SD"),
            (spec.length_scale_prior_sd_um, "length prior SD"),
            (spec.loading_b_prior_sd, "loading B prior SD"),
            (spec.noise_a_sd, "output A known noise SD"),
            (spec.noise_b_sd, "output B known noise SD"),
            (spec.jitter, "jitter"),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(BayesError::InvalidSpec(format!(
                    "{name} must be finite and positive"
                )));
            }
        }
        if !(5..=64).contains(&spec.observations.len())
            || !(1..=1_024).contains(&spec.predictions.len())
        {
            return Err(BayesError::InvalidSpec(
                "multi-output GP requires 5-64 observations and 1-1024 predictions".into(),
            ));
        }
        spec.observations
            .sort_by(|left, right| left.coordinate_id.cmp(&right.coordinate_id));
        spec.predictions
            .sort_by(|left, right| left.prediction_id.cmp(&right.prediction_id));
        let mut ids = BTreeSet::new();
        let mut coordinates = BTreeSet::new();
        let mut output_a_values = BTreeSet::new();
        let mut output_b_values = BTreeSet::new();
        for row in &spec.observations {
            if row.coordinate_id.is_empty()
                || row.coordinate_id.trim() != row.coordinate_id
                || !row.x_um.is_finite()
                || !row.output_a.is_finite()
                || !row.output_b.is_finite()
                || !ids.insert(row.coordinate_id.as_str())
                || !coordinates.insert(row.x_um.to_bits())
            {
                return Err(BayesError::InvalidSpec(
                    "multi-output rows require exact unique IDs/coordinates and finite values"
                        .into(),
                ));
            }
            output_a_values.insert(row.output_a.to_bits());
            output_b_values.insert(row.output_b.to_bits());
        }
        if output_a_values.len() < 2 || output_b_values.len() < 2 {
            return Err(BayesError::InvalidSpec(
                "each multi-output GP field must vary".into(),
            ));
        }
        let mut prediction_ids = BTreeSet::new();
        for row in &spec.predictions {
            if row.prediction_id.is_empty()
                || row.prediction_id.trim() != row.prediction_id
                || !row.x_um.is_finite()
                || !prediction_ids.insert(row.prediction_id.as_str())
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
        let n = (2 * spec.observations.len()) as u128;
        let m = (2 * spec.predictions.len()) as u128;
        let posterior_draws = u128::from(sampling.chains) * u128::from(sampling.draws_per_chain);
        let work = u128::from(total_iterations) * n.pow(3) + posterior_draws * n.pow(2) * m;
        let maximum_conditioning_work = 2_000_000_000_u64;
        if total_iterations > maximum_total_iterations
            || work > u128::from(maximum_conditioning_work)
        {
            return Err(BayesError::InvalidSpec(
                "multi-output GP exceeds the iteration/conditioning-work limit".into(),
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
            model: MultiOutputGpModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "one_factor_two_output_matern32_gp",
                coordinate_dimension: 1,
                coordinate_unit: "micrometre",
                output_a_name: spec.output_a_name,
                output_b_name: spec.output_b_name,
                latent_processes: 1,
                kernel: "matern_3_2",
                loading_a: 1.0,
                loading_b_constraint: "positive",
                mean_prior_sd: spec.mean_prior_sd,
                amplitude_prior_sd: spec.amplitude_prior_sd,
                length_scale_prior_sd_um: spec.length_scale_prior_sd_um,
                loading_b_prior_sd: spec.loading_b_prior_sd,
                noise_a_sd: spec.noise_a_sd,
                noise_b_sd: spec.noise_b_sd,
                jitter: spec.jitter,
                backend_capability: "nuts_exact_dense_multi_output_gp",
                maturity: "experimental",
            },
            observations: spec.observations,
            predictions: spec.predictions,
            sampling,
            resources: GpResourceLimits {
                maximum_observations: 64,
                maximum_predictions: 1_024,
                maximum_total_iterations,
                maximum_conditioning_work,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultiOutputScalarSummary {
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultiOutputPosterior {
    pub mean_a: MultiOutputScalarSummary,
    pub mean_b: MultiOutputScalarSummary,
    pub amplitude: MultiOutputScalarSummary,
    pub length_scale_um: MultiOutputScalarSummary,
    pub loading_b: MultiOutputScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultiOutputPrediction {
    pub prediction_id: String,
    pub x_um: f64,
    pub output_a_mean: f64,
    pub output_a_sd: f64,
    pub output_a_interval_lower: f64,
    pub output_a_interval_upper: f64,
    pub output_b_mean: f64,
    pub output_b_sd: f64,
    pub output_b_interval_lower: f64,
    pub output_b_interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultiOutputPosteriorPredictive {
    pub observed_correlation: f64,
    pub replicated_correlation_mean: f64,
    pub observed_a_mean: f64,
    pub replicated_a_mean: f64,
    pub observed_b_mean: f64,
    pub replicated_b_mean: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultiOutputGpWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: MultiOutputPosterior,
    pub predictions: Vec<MultiOutputPrediction>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: MultiOutputPosteriorPredictive,
}

impl MultiOutputGpWorkerResult {
    pub fn validate(
        &self,
        request: &MultiOutputGpWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly {
            return Err(BayesError::WorkerContract(
                "multi-output GP NUTS cannot return approximate-only state".into(),
            ));
        }
        if self.format != "marklab.pymc_multi_output_gp_worker_result"
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
                "multi-output GP result identity mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.completed_draws != expected_draws
            || self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
        {
            return Err(BayesError::WorkerContract(
                "multi-output GP sampling counts mismatch".into(),
            ));
        }
        for summary in [
            &self.posterior.mean_a,
            &self.posterior.mean_b,
            &self.posterior.amplitude,
            &self.posterior.length_scale_um,
            &self.posterior.loading_b,
        ] {
            validate_scalar(summary)?;
        }
        if self.posterior.amplitude.mean <= 0.0
            || self.posterior.amplitude.interval_lower < 0.0
            || self.posterior.length_scale_um.mean <= 0.0
            || self.posterior.length_scale_um.interval_lower < 0.0
            || self.posterior.loading_b.mean <= 0.0
            || self.posterior.loading_b.interval_lower < 0.0
        {
            return Err(BayesError::WorkerContract(
                "invalid multi-output GP positive parameter".into(),
            ));
        }
        for (actual, expected) in self.predictions.iter().zip(&request.predictions) {
            if actual.prediction_id != expected.prediction_id
                || actual.x_um.to_bits() != expected.x_um.to_bits()
                || !finite(&[
                    actual.output_a_mean,
                    actual.output_a_sd,
                    actual.output_a_interval_lower,
                    actual.output_a_interval_upper,
                    actual.output_b_mean,
                    actual.output_b_sd,
                    actual.output_b_interval_lower,
                    actual.output_b_interval_upper,
                ])
                || actual.output_a_sd <= 0.0
                || actual.output_b_sd <= 0.0
                || actual.output_a_interval_lower > actual.output_a_interval_upper
                || actual.output_b_interval_lower > actual.output_b_interval_upper
            {
                return Err(BayesError::WorkerContract(
                    "invalid multi-output prediction".into(),
                ));
            }
        }
        if !finite(&[
            self.diagnostics.r_hat,
            self.diagnostics.ess_bulk,
            self.diagnostics.ess_tail,
            self.diagnostics.mcse_mean,
            self.diagnostics.mcse_sd,
            self.diagnostics.minimum_ebfmi,
            self.posterior_predictive.observed_correlation,
            self.posterior_predictive.replicated_correlation_mean,
            self.posterior_predictive.observed_a_mean,
            self.posterior_predictive.replicated_a_mean,
            self.posterior_predictive.observed_b_mean,
            self.posterior_predictive.replicated_b_mean,
        ]) || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
            || !(-1.0..=1.0).contains(&self.posterior_predictive.observed_correlation)
            || !(-1.0..=1.0).contains(&self.posterior_predictive.replicated_correlation_mean)
        {
            return Err(BayesError::WorkerContract(
                "invalid multi-output diagnostics".into(),
            ));
        }
        let observed_a = request
            .observations
            .iter()
            .map(|row| row.output_a)
            .collect::<Vec<_>>();
        let observed_b = request
            .observations
            .iter()
            .map(|row| row.output_b)
            .collect::<Vec<_>>();
        let mean_a = observed_a.iter().sum::<f64>() / observed_a.len() as f64;
        let mean_b = observed_b.iter().sum::<f64>() / observed_b.len() as f64;
        let numerator = observed_a
            .iter()
            .zip(&observed_b)
            .map(|(a, b)| (a - mean_a) * (b - mean_b))
            .sum::<f64>();
        let denominator = (observed_a.iter().map(|a| (a - mean_a).powi(2)).sum::<f64>()
            * observed_b.iter().map(|b| (b - mean_b).powi(2)).sum::<f64>())
        .sqrt();
        let correlation = numerator / denominator;
        if (self.posterior_predictive.observed_a_mean - mean_a).abs()
            > 1e-12 * mean_a.abs().max(1.0)
            || (self.posterior_predictive.observed_b_mean - mean_b).abs()
                > 1e-12 * mean_b.abs().max(1.0)
            || (self.posterior_predictive.observed_correlation - correlation).abs() > 1e-12
        {
            return Err(BayesError::WorkerContract(
                "multi-output posterior predictive changed observed summaries".into(),
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
                "multi-output fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: MultiOutputGpWorkerRequest,
        input: MultiOutputGpInputIdentity,
    ) -> MultiOutputGpFit {
        MultiOutputGpFit {
            format: "marklab.bayesian_multi_output_gp_fit",
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

fn validate_scalar(value: &MultiOutputScalarSummary) -> Result<(), BayesError> {
    if !finite(&[
        value.mean,
        value.sd,
        value.interval_lower,
        value.interval_upper,
    ]) || value.sd <= 0.0
        || value.interval_lower > value.interval_upper
    {
        return Err(BayesError::WorkerContract(
            "invalid multi-output scalar summary".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct MultiOutputGpInputIdentity {
    pub observation_path: String,
    pub prediction_path: String,
    pub observations: usize,
    pub predictions: usize,
    pub data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct MultiOutputGpFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: MultiOutputGpModelIr,
    pub input: MultiOutputGpInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: MultiOutputPosterior,
    pub predictions: Vec<MultiOutputPrediction>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: MultiOutputPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

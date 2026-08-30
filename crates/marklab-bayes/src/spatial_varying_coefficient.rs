use crate::validation::all_finite as finite;

use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, DiagnosticPolicy, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION,
        PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    sar_fit::design_full_rank,
    BayesError, FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary,
    SarScalarSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct SpatialCoefficientObservation {
    pub coordinate_id: String,
    pub x_um: f64,
    pub outcome: f64,
    pub global_predictor: f64,
    pub spatial_predictor: f64,
}

#[derive(Clone, Debug)]
pub struct SpatialVaryingCoefficientSpec {
    pub global_predictor_name: String,
    pub spatial_predictor_name: String,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: f64,
    pub known_noise_sd: f64,
    pub jitter: f64,
    pub observations: Vec<SpatialCoefficientObservation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpatialVaryingCoefficientModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub kernel: &'static str,
    pub coordinate_unit: &'static str,
    pub field_constraint: &'static str,
    pub global_predictor_name: String,
    pub spatial_predictor_name: String,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub amplitude_prior_sd: f64,
    pub length_scale_prior_sd_um: f64,
    pub known_noise_sd: f64,
    pub jitter: f64,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpatialVaryingCoefficientWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: SpatialVaryingCoefficientModelIr,
    pub observations: Vec<SpatialCoefficientObservation>,
    pub centering_basis: Vec<f64>,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl SpatialVaryingCoefficientWorkerRequest {
    pub fn new(
        mut spec: SpatialVaryingCoefficientSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        validate_spec(&mut spec)?;
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "spatial coefficient worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let dimension = spec.observations.len();
        let total_iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        let maximum_total_iterations = 400_000;
        let work = total_iterations
            .checked_mul(dimension as u64)
            .and_then(|value| value.checked_mul(dimension as u64))
            .and_then(|value| value.checked_mul(dimension as u64))
            .ok_or_else(|| BayesError::InvalidSpec("spatial coefficient work overflow".into()))?;
        if total_iterations > maximum_total_iterations || work > 2_000_000_000 {
            return Err(BayesError::InvalidSpec(
                "spatial coefficient NUTS or dense work exceeds its bound".into(),
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
            model: SpatialVaryingCoefficientModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "gaussian_spatially_varying_coefficient",
                kernel: "matern_3_2",
                coordinate_unit: "micrometre",
                field_constraint: "exact_sum_to_zero_orthonormal_basis",
                global_predictor_name: spec.global_predictor_name,
                spatial_predictor_name: spec.spatial_predictor_name,
                intercept_prior_mean: spec.intercept_prior_mean,
                intercept_prior_sd: spec.intercept_prior_sd,
                coefficient_prior_sd: spec.coefficient_prior_sd,
                amplitude_prior_sd: spec.amplitude_prior_sd,
                length_scale_prior_sd_um: spec.length_scale_prior_sd_um,
                known_noise_sd: spec.known_noise_sd,
                jitter: spec.jitter,
                backend_capability: "nuts",
                maturity: "experimental",
            },
            centering_basis: helmert_basis(dimension),
            observations: spec.observations,
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

fn validate_spec(spec: &mut SpatialVaryingCoefficientSpec) -> Result<(), BayesError> {
    if !(8..=64).contains(&spec.observations.len()) {
        return Err(BayesError::InvalidSpec(
            "spatial coefficient model requires 8-64 observations".into(),
        ));
    }
    for name in [&spec.global_predictor_name, &spec.spatial_predictor_name] {
        if name.is_empty() || name.trim() != name {
            return Err(BayesError::InvalidSpec(
                "spatial coefficient predictor names must be exact and nonempty".into(),
            ));
        }
    }
    if spec.global_predictor_name == spec.spatial_predictor_name {
        return Err(BayesError::InvalidSpec(
            "global and spatial predictor names must differ".into(),
        ));
    }
    for (value, name) in [
        (spec.intercept_prior_sd, "intercept prior SD"),
        (spec.coefficient_prior_sd, "coefficient prior SD"),
        (spec.amplitude_prior_sd, "amplitude prior SD"),
        (spec.length_scale_prior_sd_um, "length-scale prior SD"),
        (spec.known_noise_sd, "known noise SD"),
        (spec.jitter, "jitter"),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(BayesError::InvalidSpec(format!(
                "spatial coefficient {name} must be finite and positive"
            )));
        }
    }
    if !spec.intercept_prior_mean.is_finite() {
        return Err(BayesError::InvalidSpec(
            "spatial coefficient intercept prior mean must be finite".into(),
        ));
    }
    spec.observations
        .sort_by(|left, right| left.x_um.total_cmp(&right.x_um));
    for (index, observation) in spec.observations.iter().enumerate() {
        if observation.coordinate_id.is_empty()
            || observation.coordinate_id.trim() != observation.coordinate_id
            || !observation.x_um.is_finite()
            || !observation.outcome.is_finite()
            || !observation.global_predictor.is_finite()
            || !observation.spatial_predictor.is_finite()
            || (index > 0 && spec.observations[index - 1].x_um == observation.x_um)
            || spec.observations[..index]
                .iter()
                .any(|prior| prior.coordinate_id == observation.coordinate_id)
        {
            return Err(BayesError::InvalidSpec(
                "spatial coefficient observations require unique exact IDs/coordinates and finite values"
                    .into(),
            ));
        }
    }
    let design = spec
        .observations
        .iter()
        .flat_map(|observation| [observation.global_predictor, observation.spatial_predictor])
        .collect::<Vec<_>>();
    if !design_full_rank(&design, spec.observations.len(), 2) {
        return Err(BayesError::InvalidSpec(
            "spatial coefficient fixed design including intercept must be full rank".into(),
        ));
    }
    Ok(())
}

fn helmert_basis(dimension: usize) -> Vec<f64> {
    let columns = dimension - 1;
    let mut basis = vec![0.0; dimension * columns];
    for column in 0..columns {
        let denominator = (((column + 1) * (column + 2)) as f64).sqrt();
        for row in 0..=column {
            basis[row * columns + column] = denominator.recip();
        }
        basis[(column + 1) * columns + column] = -(column as f64 + 1.0) / denominator;
    }
    basis
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialCoefficientPosterior {
    pub intercept: SarScalarSummary,
    pub global_coefficient: SarScalarSummary,
    pub spatial_mean_coefficient: SarScalarSummary,
    pub amplitude: SarScalarSummary,
    pub length_scale_um: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialCoefficientFieldSummary {
    pub coordinate_id: String,
    pub x_um: f64,
    pub deviation: SarScalarSummary,
    pub varying_coefficient: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialCoefficientPosteriorPredictive {
    pub observed_mean: f64,
    pub observed_sd: f64,
    pub replicated_mean_mean: f64,
    pub replicated_mean_sd: f64,
    pub replicated_sd_mean: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialVaryingCoefficientWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: SpatialCoefficientPosterior,
    pub coefficient_field: Vec<SpatialCoefficientFieldSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: SpatialCoefficientPosteriorPredictive,
}

impl SpatialVaryingCoefficientWorkerResult {
    pub fn validate(
        &self,
        request: &SpatialVaryingCoefficientWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly
            || self.format != "marklab.pymc_spatial_varying_coefficient_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "spatial coefficient result identity mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
            || self.coefficient_field.len() != request.observations.len()
        {
            return Err(BayesError::WorkerContract(
                "spatial coefficient sampling or field counts mismatch".into(),
            ));
        }
        for summary in [
            &self.posterior.intercept,
            &self.posterior.global_coefficient,
            &self.posterior.spatial_mean_coefficient,
            &self.posterior.amplitude,
            &self.posterior.length_scale_um,
        ] {
            validate_scalar(summary, false)?;
        }
        if self.posterior.amplitude.mean <= 0.0
            || self.posterior.amplitude.interval_lower < 0.0
            || self.posterior.length_scale_um.mean <= 0.0
            || self.posterior.length_scale_um.interval_lower < 0.0
        {
            return Err(BayesError::WorkerContract(
                "spatial coefficient kernel posterior violates support".into(),
            ));
        }
        for (actual, expected) in self.coefficient_field.iter().zip(&request.observations) {
            if actual.coordinate_id != expected.coordinate_id
                || actual.x_um.to_bits() != expected.x_um.to_bits()
            {
                return Err(BayesError::WorkerContract(
                    "spatial coefficient field changed coordinate identity".into(),
                ));
            }
            validate_scalar(&actual.deviation, false)?;
            validate_scalar(&actual.varying_coefficient, false)?;
        }
        let deviation_sum = self
            .coefficient_field
            .iter()
            .map(|field| field.deviation.mean)
            .sum::<f64>();
        let observed_mean = request
            .observations
            .iter()
            .map(|observation| observation.outcome)
            .sum::<f64>()
            / request.observations.len() as f64;
        let observed_sd = (request
            .observations
            .iter()
            .map(|observation| (observation.outcome - observed_mean).powi(2))
            .sum::<f64>()
            / (request.observations.len() - 1) as f64)
            .sqrt();
        if deviation_sum.abs() > 1e-10
            || !finite(&[
                self.posterior_predictive.observed_mean,
                self.posterior_predictive.observed_sd,
                self.posterior_predictive.replicated_mean_mean,
                self.posterior_predictive.replicated_mean_sd,
                self.posterior_predictive.replicated_sd_mean,
                self.diagnostics.r_hat,
                self.diagnostics.ess_bulk,
                self.diagnostics.ess_tail,
                self.diagnostics.mcse_mean,
                self.diagnostics.mcse_sd,
                self.diagnostics.minimum_ebfmi,
            ])
            || (self.posterior_predictive.observed_mean - observed_mean).abs()
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
                "spatial coefficient centering, predictive, or diagnostics are invalid".into(),
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
                "spatial coefficient fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: SpatialVaryingCoefficientWorkerRequest,
        input: SpatialCoefficientInputIdentity,
    ) -> SpatialVaryingCoefficientFit {
        SpatialVaryingCoefficientFit {
            format: "marklab.bayesian_spatial_varying_coefficient_fit",
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
            constraints: vec!["sum_to_zero:delta"],
            coefficient_field: self.coefficient_field,
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
            "invalid spatial coefficient scalar summary".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct SpatialCoefficientInputIdentity {
    pub path: String,
    pub observations_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct SpatialVaryingCoefficientFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: SpatialVaryingCoefficientModelIr,
    pub input: SpatialCoefficientInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: SpatialCoefficientPosterior,
    pub constraints: Vec<&'static str>,
    pub coefficient_field: Vec<SpatialCoefficientFieldSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: SpatialCoefficientPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_confounded_fixed_design() {
        let mut observations = Vec::new();
        for index in 0..8 {
            observations.push(SpatialCoefficientObservation {
                coordinate_id: format!("p{index}"),
                x_um: index as f64,
                outcome: index as f64,
                global_predictor: index as f64,
                spatial_predictor: index as f64,
            });
        }
        assert!(matches!(
            SpatialVaryingCoefficientWorkerRequest::new(
                spec(observations),
                sampling(),
                "lock".into(),
                "worker".into(),
                180,
            ),
            Err(BayesError::InvalidSpec(_))
        ));
    }

    fn spec(observations: Vec<SpatialCoefficientObservation>) -> SpatialVaryingCoefficientSpec {
        SpatialVaryingCoefficientSpec {
            global_predictor_name: "global".into(),
            spatial_predictor_name: "spatial".into(),
            intercept_prior_mean: 0.0,
            intercept_prior_sd: 2.0,
            coefficient_prior_sd: 2.0,
            amplitude_prior_sd: 1.0,
            length_scale_prior_sd_um: 5.0,
            known_noise_sd: 0.1,
            jitter: 1e-6,
            observations,
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

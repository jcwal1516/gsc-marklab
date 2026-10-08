use crate::validation::all_finite as finite;

use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, DiagnosticPolicy, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION,
        WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, NutsSamplingSpec, PoissonExposureObservation, SarScalarSummary,
    WorkerBackend,
};

const INLA_BACKEND_VERSION: &str = "scipy-1.18.1+pytensor-3.2.4+pymc-6.3.0";

#[derive(Clone, Debug)]
pub struct PoissonInlaSpec {
    pub latent_mean: f64,
    pub tau_shape: f64,
    pub tau_rate: f64,
    pub log_tau_min: f64,
    pub log_tau_max: f64,
    pub grid_points: u32,
    pub endpoint_mass_limit: f64,
    pub observations: Vec<PoissonExposureObservation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PoissonInlaModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub latent_field: &'static str,
    pub latent_mean: f64,
    pub hyperparameter: &'static str,
    pub tau_prior: &'static str,
    pub tau_shape: f64,
    pub tau_rate: f64,
    pub likelihood: &'static str,
    pub link: &'static str,
    pub integration_coordinate: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct InlaGridSpec {
    pub log_tau_min: f64,
    pub log_tau_max: f64,
    pub points: u32,
    pub endpoint_mass_limit: f64,
    pub mode_gradient_tolerance: f64,
    pub maximum_mode_iterations: u32,
    pub hmc_mean_rmse_limit: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PoissonInlaWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: PoissonInlaModelIr,
    pub observations: Vec<PoissonExposureObservation>,
    pub grid: InlaGridSpec,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl PoissonInlaWorkerRequest {
    pub fn new(
        mut spec: PoissonInlaSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        validate_spec(&mut spec)?;
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "INLA-style worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let total_iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        let maximum_total_iterations = 400_000;
        if total_iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "INLA comparison NUTS iterations exceed 400000".into(),
            ));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "scipy_pytensor_pymc",
                version: INLA_BACKEND_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: PoissonInlaModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "poisson_lognormal_latent_gaussian",
                latent_field: "conditionally_independent_normal_log_rates",
                latent_mean: spec.latent_mean,
                hyperparameter: "precision_tau",
                tau_prior: "gamma_shape_rate",
                tau_shape: spec.tau_shape,
                tau_rate: spec.tau_rate,
                likelihood: "poisson_exposure",
                link: "log",
                integration_coordinate: "log_tau_with_jacobian",
                backend_capability: "inla_style_nested_laplace_with_nuts_comparison",
                maturity: "experimental_approximation",
            },
            observations: spec.observations,
            grid: InlaGridSpec {
                log_tau_min: spec.log_tau_min,
                log_tau_max: spec.log_tau_max,
                points: spec.grid_points,
                endpoint_mass_limit: spec.endpoint_mass_limit,
                mode_gradient_tolerance: 1e-10,
                maximum_mode_iterations: 1_000,
                hmc_mean_rmse_limit: 0.15,
            },
            sampling,
            resources: WorkerResourceLimits {
                maximum_observations: 16,
                maximum_total_iterations,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

fn validate_spec(spec: &mut PoissonInlaSpec) -> Result<(), BayesError> {
    if !(3..=16).contains(&spec.observations.len())
        || !spec.latent_mean.is_finite()
        || !spec.tau_shape.is_finite()
        || spec.tau_shape <= 0.0
        || !spec.tau_rate.is_finite()
        || spec.tau_rate <= 0.0
        || !spec.log_tau_min.is_finite()
        || !spec.log_tau_max.is_finite()
        || spec.log_tau_min >= spec.log_tau_max
        || !(21..=201).contains(&spec.grid_points)
        || !spec.endpoint_mass_limit.is_finite()
        || spec.endpoint_mass_limit <= 0.0
        || spec.endpoint_mass_limit >= 0.5
    {
        return Err(BayesError::InvalidSpec(
            "INLA-style model, grid, or hyperprior controls are invalid".into(),
        ));
    }
    spec.observations
        .sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let mut count_total = 0_u64;
    let mut exposure_total = 0.0;
    for (index, observation) in spec.observations.iter().enumerate() {
        if observation.observation_id.is_empty()
            || observation.observation_id.trim() != observation.observation_id
            || !observation.exposure.is_finite()
            || observation.exposure <= 0.0
            || observation.count > i64::MAX as u64
            || (index > 0
                && spec.observations[index - 1].observation_id == observation.observation_id)
        {
            return Err(BayesError::InvalidSpec(
                "INLA-style observations require unique exact IDs and valid count/exposure values"
                    .into(),
            ));
        }
        count_total = count_total
            .checked_add(observation.count)
            .ok_or_else(|| BayesError::InvalidSpec("INLA-style aggregate count overflow".into()))?;
        exposure_total += observation.exposure;
    }
    if count_total > i64::MAX as u64 || !exposure_total.is_finite() {
        return Err(BayesError::InvalidSpec(
            "INLA-style aggregate count or exposure exceeds backend range".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InlaGridPoint {
    pub log_tau: f64,
    pub tau: f64,
    pub log_unnormalized_density: f64,
    pub normalized_weight: f64,
    pub maximum_mode_gradient: f64,
    pub minimum_negative_hessian: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InlaGridDiagnostics {
    pub step: f64,
    pub log_normalizing_constant: f64,
    pub weight_sum: f64,
    pub left_endpoint_mass: f64,
    pub right_endpoint_mass: f64,
    pub all_modes_converged: bool,
    pub all_hessians_positive: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InlaLatentMarginal {
    pub region_id: String,
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
    pub hmc_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InlaHmcComparison {
    pub fit_state: FitState,
    pub latent_mean_rmse: f64,
    pub r_hat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
    pub minimum_ebfmi: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InlaPosteriorPredictive {
    pub observed_total_count: u64,
    pub approximated_total_mean: f64,
    pub hmc_replicated_total_mean: f64,
    pub hmc_replicated_total_sd: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoissonInlaWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub grid: Vec<InlaGridPoint>,
    pub grid_diagnostics: InlaGridDiagnostics,
    pub tau: SarScalarSummary,
    pub latent_marginals: Vec<InlaLatentMarginal>,
    pub hmc_comparison: InlaHmcComparison,
    pub posterior_predictive: InlaPosteriorPredictive,
}

impl PoissonInlaWorkerResult {
    pub fn validate(
        &self,
        request: &PoissonInlaWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::Complete
            || self.format != "marklab.scipy_pytensor_pymc_inla_worker_result"
            || self.version != 1
            || self.backend.name != "scipy_pytensor_pymc"
            || self.backend.version != INLA_BACKEND_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.grid.len() != request.grid.points as usize
            || self.latent_marginals.len() != request.observations.len()
        {
            return Err(BayesError::WorkerContract(
                "INLA-style result identity or dimensions mismatch".into(),
            ));
        }
        let expected_step = (request.grid.log_tau_max - request.grid.log_tau_min)
            / f64::from(request.grid.points - 1);
        let mut prior_log_tau = f64::NEG_INFINITY;
        let mut weight_sum = 0.0;
        let mut all_modes_converged = true;
        for (index, point) in self.grid.iter().enumerate() {
            let expected_log_tau = request.grid.log_tau_min + index as f64 * expected_step;
            if !finite(&[
                point.log_tau,
                point.tau,
                point.log_unnormalized_density,
                point.normalized_weight,
                point.maximum_mode_gradient,
                point.minimum_negative_hessian,
            ]) || point.log_tau <= prior_log_tau
                || point.tau <= 0.0
                || point.normalized_weight < 0.0
                || point.maximum_mode_gradient < 0.0
                || point.minimum_negative_hessian <= 0.0
                || !approximately_equal(point.log_tau, expected_log_tau)
                || !approximately_equal(point.tau, point.log_tau.exp())
            {
                return Err(BayesError::WorkerContract(
                    "INLA-style grid point is invalid".into(),
                ));
            }
            prior_log_tau = point.log_tau;
            weight_sum += point.normalized_weight;
            all_modes_converged &=
                point.maximum_mode_gradient <= request.grid.mode_gradient_tolerance;
        }
        let hmc_diagnostics_pass = self.hmc_comparison.r_hat
            <= request.diagnostic_policy.maximum_r_hat
            && self.hmc_comparison.ess_bulk >= request.diagnostic_policy.minimum_bulk_ess
            && self.hmc_comparison.ess_tail >= request.diagnostic_policy.minimum_tail_ess
            && self.hmc_comparison.minimum_ebfmi >= request.diagnostic_policy.minimum_ebfmi
            && self.hmc_comparison.divergences <= request.diagnostic_policy.maximum_divergences
            && self.hmc_comparison.max_tree_depth_hits
                <= request.diagnostic_policy.maximum_tree_depth_hits;
        if !finite(&[
            self.grid_diagnostics.step,
            self.grid_diagnostics.log_normalizing_constant,
            self.grid_diagnostics.weight_sum,
            self.grid_diagnostics.left_endpoint_mass,
            self.grid_diagnostics.right_endpoint_mass,
            self.tau.mean,
            self.tau.sd,
            self.tau.interval_lower,
            self.tau.interval_upper,
            self.hmc_comparison.latent_mean_rmse,
            self.hmc_comparison.r_hat,
            self.hmc_comparison.ess_bulk,
            self.hmc_comparison.ess_tail,
            self.hmc_comparison.minimum_ebfmi,
            self.posterior_predictive.approximated_total_mean,
            self.posterior_predictive.hmc_replicated_total_mean,
            self.posterior_predictive.hmc_replicated_total_sd,
        ]) || (weight_sum - 1.0).abs() > 1e-12
            || (self.grid_diagnostics.weight_sum - weight_sum).abs() > 1e-12
            || !approximately_equal(self.grid_diagnostics.step, expected_step)
            || !approximately_equal(
                self.grid_diagnostics.left_endpoint_mass,
                self.grid[0].normalized_weight,
            )
            || !approximately_equal(
                self.grid_diagnostics.right_endpoint_mass,
                self.grid[self.grid.len() - 1].normalized_weight,
            )
            || self.grid_diagnostics.all_modes_converged != all_modes_converged
            || !self.grid_diagnostics.all_hessians_positive
            || self.grid_diagnostics.left_endpoint_mass < 0.0
            || self.grid_diagnostics.right_endpoint_mass < 0.0
            || self.tau.mean <= 0.0
            || self.tau.sd <= 0.0
            || self.tau.interval_lower <= 0.0
            || self.tau.interval_lower > self.tau.interval_upper
            || self.hmc_comparison.fit_state == FitState::ApproximateOnly
            || (self.hmc_comparison.fit_state == FitState::Complete && !hmc_diagnostics_pass)
            || self.hmc_comparison.latent_mean_rmse < 0.0
            || self.hmc_comparison.r_hat <= 0.0
            || self.hmc_comparison.ess_bulk <= 0.0
            || self.hmc_comparison.ess_tail <= 0.0
            || self.hmc_comparison.minimum_ebfmi < 0.0
            || self.posterior_predictive.approximated_total_mean < 0.0
            || self.posterior_predictive.hmc_replicated_total_mean < 0.0
            || self.posterior_predictive.hmc_replicated_total_sd < 0.0
        {
            return Err(BayesError::WorkerContract(
                "INLA-style grid, tau, HMC, or predictive summary is invalid".into(),
            ));
        }
        for (actual, expected) in self.latent_marginals.iter().zip(&request.observations) {
            if actual.region_id != expected.observation_id
                || !finite(&[
                    actual.mean,
                    actual.sd,
                    actual.interval_lower,
                    actual.interval_upper,
                    actual.hmc_mean,
                ])
                || actual.sd <= 0.0
                || actual.interval_lower > actual.interval_upper
            {
                return Err(BayesError::WorkerContract(
                    "INLA-style latent marginal is invalid".into(),
                ));
            }
        }
        let observed_total = request
            .observations
            .iter()
            .map(|row| row.count)
            .sum::<u64>();
        if self.posterior_predictive.observed_total_count != observed_total {
            return Err(BayesError::WorkerContract(
                "INLA-style predictive changed observed total".into(),
            ));
        }
        let approximation_valid = self.grid_diagnostics.all_modes_converged
            && self.grid_diagnostics.all_hessians_positive
            && self.grid_diagnostics.left_endpoint_mass <= request.grid.endpoint_mass_limit
            && self.grid_diagnostics.right_endpoint_mass <= request.grid.endpoint_mass_limit
            && self.hmc_comparison.fit_state == FitState::Complete
            && self.hmc_comparison.latent_mean_rmse <= request.grid.hmc_mean_rmse_limit;
        if (self.fit_state == FitState::ApproximateOnly) != approximation_valid {
            return Err(BayesError::WorkerContract(
                "INLA-style fit state disagrees with approximation gates".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: PoissonInlaWorkerRequest,
        input: PoissonInlaInputIdentity,
    ) -> PoissonInlaFit {
        PoissonInlaFit {
            format: "marklab.bayesian_poisson_lognormal_inla",
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
            grid_spec: request.grid,
            grid: self.grid,
            grid_diagnostics: self.grid_diagnostics,
            tau: self.tau,
            latent_marginals: self.latent_marginals,
            hmc_comparison: self.hmc_comparison,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12 * left.abs().max(right.abs()).max(1.0)
}

#[derive(Debug, Serialize)]
pub struct PoissonInlaInputIdentity {
    pub path: String,
    pub region_count: usize,
    pub observations_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PoissonInlaFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: PoissonInlaModelIr,
    pub input: PoissonInlaInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub grid_spec: InlaGridSpec,
    pub grid: Vec<InlaGridPoint>,
    pub grid_diagnostics: InlaGridDiagnostics,
    pub tau: SarScalarSummary,
    pub latent_marginals: Vec<InlaLatentMarginal>,
    pub hmc_comparison: InlaHmcComparison,
    pub posterior_predictive: InlaPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

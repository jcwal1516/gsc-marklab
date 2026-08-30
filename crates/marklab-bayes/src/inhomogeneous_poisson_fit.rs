use crate::validation::all_finite as finite;

use serde::{Deserialize, Serialize};

use crate::{
    inhomogeneous_poisson::validate_and_canonicalize,
    model::{
        BackendContract, DiagnosticPolicy, MODEL_FORMAT, MODEL_VERSION, PYMC_VERSION,
        WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, InhomogeneousPoissonEvent, InhomogeneousPoissonSpec,
    MidpointQuadratureValue, NormalMeanDiagnostics, NutsSamplingSpec, RectangularWindow,
    SamplingSummary, SarScalarSummary, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct InhomogeneousPoissonFitSpec {
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub events: Vec<InhomogeneousPoissonEvent>,
    pub quadrature: Vec<MidpointQuadratureValue>,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_mean: f64,
    pub coefficient_prior_sd: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct InhomogeneousPoissonFitModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub coordinate_unit: &'static str,
    pub window: &'static str,
    pub quadrature: &'static str,
    pub covariates: &'static str,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_mean: f64,
    pub coefficient_prior_sd: f64,
    pub likelihood: &'static str,
    pub posterior_predictive: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct InhomogeneousPoissonFitResourceLimits {
    pub maximum_events: u32,
    pub maximum_quadrature_nodes: u32,
    pub maximum_total_iterations: u64,
    pub maximum_draw_cell_work: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct InhomogeneousPoissonFitWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: InhomogeneousPoissonFitModelIr,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub cell_width_um: f64,
    pub cell_height_um: f64,
    pub cell_area_um2: f64,
    pub events: Vec<InhomogeneousPoissonEvent>,
    pub quadrature: Vec<MidpointQuadratureValue>,
    pub observed_cell_counts: Vec<u64>,
    pub sampling: NutsSamplingSpec,
    pub resources: InhomogeneousPoissonFitResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl InhomogeneousPoissonFitWorkerRequest {
    pub fn new(
        spec: InhomogeneousPoissonFitSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if !(1..=3_600).contains(&timeout_seconds)
            || !spec.intercept_prior_mean.is_finite()
            || !spec.intercept_prior_sd.is_finite()
            || spec.intercept_prior_sd <= 0.0
            || !spec.coefficient_prior_mean.is_finite()
            || !spec.coefficient_prior_sd.is_finite()
            || spec.coefficient_prior_sd <= 0.0
        {
            return Err(BayesError::InvalidSpec(
                "inhomogeneous Poisson fit priors or timeout are invalid".into(),
            ));
        }
        let mut validated = InhomogeneousPoissonSpec {
            window: spec.window,
            grid_x: spec.grid_x,
            grid_y: spec.grid_y,
            events: spec.events,
            quadrature: spec.quadrature,
            intercept: 0.0,
            coefficient: 0.0,
        };
        validate_and_canonicalize(&mut validated)
            .map_err(|error| BayesError::InvalidSpec(error.to_string()))?;
        let node_count = validated.quadrature.len();
        if !(20..=100_000).contains(&validated.events.len()) || !(4..=4_096).contains(&node_count) {
            return Err(BayesError::InvalidSpec(
                "inhomogeneous Poisson fitting requires 20-100000 events and 4-4096 grid cells"
                    .into(),
            ));
        }
        let covariate_min = validated
            .quadrature
            .iter()
            .map(|node| node.covariate)
            .fold(f64::INFINITY, f64::min);
        let covariate_max = validated
            .quadrature
            .iter()
            .map(|node| node.covariate)
            .fold(f64::NEG_INFINITY, f64::max);
        let covariate_scale = covariate_min.abs().max(covariate_max.abs()).max(1.0);
        if covariate_max - covariate_min <= f64::EPSILON.sqrt() * covariate_scale {
            return Err(BayesError::InvalidSpec(
                "inhomogeneous Poisson fit quadrature covariate must vary materially".into(),
            ));
        }
        let width = validated.window.xmax_um - validated.window.xmin_um;
        let height = validated.window.ymax_um - validated.window.ymin_um;
        let cell_width_um = width / f64::from(validated.grid_x);
        let cell_height_um = height / f64::from(validated.grid_y);
        let cell_area_um2 = cell_width_um * cell_height_um;
        let mut observed_cell_counts = vec![0_u64; node_count];
        for event in &validated.events {
            let ix = ((event.x_um - validated.window.xmin_um) / cell_width_um).floor() as usize;
            let iy = ((event.y_um - validated.window.ymin_um) / cell_height_um).floor() as usize;
            let index = iy * validated.grid_x as usize + ix;
            observed_cell_counts[index] += 1;
        }
        let maximum_total_iterations = 400_000;
        let iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        if iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "inhomogeneous Poisson NUTS iterations exceed 400000".into(),
            ));
        }
        let completed_draws = u64::from(sampling.chains) * u64::from(sampling.draws_per_chain);
        let maximum_draw_cell_work = 20_000_000;
        if completed_draws * node_count as u64 > maximum_draw_cell_work {
            return Err(BayesError::InvalidSpec(
                "inhomogeneous Poisson posterior cell work exceeds 20000000".into(),
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
            model: InhomogeneousPoissonFitModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "log_linear_inhomogeneous_poisson_process",
                coordinate_unit: "micrometer",
                window: "half_open_rectangle",
                quadrature: "complete_regular_midpoint_equal_area",
                covariates: "one_event_and_grid_covariate_plus_offset",
                intercept_prior_mean: spec.intercept_prior_mean,
                intercept_prior_sd: spec.intercept_prior_sd,
                coefficient_prior_mean: spec.coefficient_prior_mean,
                coefficient_prior_sd: spec.coefficient_prior_sd,
                likelihood: "event_linear_predictor_sum_minus_complete_grid_integral",
                posterior_predictive: "piecewise_constant_grid_poisson_counts",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            window: validated.window,
            grid_x: validated.grid_x,
            grid_y: validated.grid_y,
            cell_width_um,
            cell_height_um,
            cell_area_um2,
            events: validated.events,
            quadrature: validated.quadrature,
            observed_cell_counts,
            sampling,
            resources: InhomogeneousPoissonFitResourceLimits {
                maximum_events: 100_000,
                maximum_quadrature_nodes: 4_096,
                maximum_total_iterations,
                maximum_draw_cell_work,
                maximum_output_bytes: 4 * 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousPoissonPosterior {
    pub intercept: SarScalarSummary,
    pub coefficient: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousPoissonCellSummary {
    pub ix: u32,
    pub iy: u32,
    pub midpoint_x_um: f64,
    pub midpoint_y_um: f64,
    pub covariate: f64,
    pub offset: f64,
    pub observed_count: u64,
    pub intensity: SarScalarSummary,
    pub expected_count: SarScalarSummary,
    pub pearson_residual: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousPoissonPosteriorPredictive {
    pub observed_total_count: u64,
    pub observed_zero_cells: u64,
    pub replicated_total_mean: f64,
    pub replicated_total_sd: f64,
    pub replicated_zero_cells_mean: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousPoissonFitWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: InhomogeneousPoissonPosterior,
    pub cells: Vec<InhomogeneousPoissonCellSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: InhomogeneousPoissonPosteriorPredictive,
}

impl InhomogeneousPoissonFitWorkerResult {
    pub fn validate(
        &self,
        request: &InhomogeneousPoissonFitWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly
            || self.format != "marklab.pymc_inhomogeneous_poisson_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.cells.len() != request.quadrature.len()
        {
            return Err(BayesError::WorkerContract(
                "inhomogeneous Poisson fit identity or dimensions mismatch".into(),
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
                "inhomogeneous Poisson sampling counts mismatch".into(),
            ));
        }
        validate_scalar(&self.posterior.intercept, false)?;
        validate_scalar(&self.posterior.coefficient, false)?;
        for (index, (cell, node)) in self.cells.iter().zip(&request.quadrature).enumerate() {
            let expected_x =
                request.window.xmin_um + (f64::from(node.ix) + 0.5) * request.cell_width_um;
            let expected_y =
                request.window.ymin_um + (f64::from(node.iy) + 0.5) * request.cell_height_um;
            if cell.ix != node.ix
                || cell.iy != node.iy
                || !approximately_equal(cell.midpoint_x_um, expected_x)
                || !approximately_equal(cell.midpoint_y_um, expected_y)
                || !approximately_equal(cell.covariate, node.covariate)
                || !approximately_equal(cell.offset, node.offset)
                || cell.observed_count != request.observed_cell_counts[index]
            {
                return Err(BayesError::WorkerContract(
                    "inhomogeneous Poisson cell identity mismatch".into(),
                ));
            }
            validate_scalar(&cell.intensity, true)?;
            validate_scalar(&cell.expected_count, true)?;
            validate_scalar(&cell.pearson_residual, false)?;
        }
        let observed_total = request.observed_cell_counts.iter().sum::<u64>();
        let observed_zeros = request
            .observed_cell_counts
            .iter()
            .filter(|count| **count == 0)
            .count() as u64;
        if self.posterior_predictive.observed_total_count != observed_total
            || self.posterior_predictive.observed_zero_cells != observed_zeros
            || !finite(&[
                self.diagnostics.r_hat,
                self.diagnostics.ess_bulk,
                self.diagnostics.ess_tail,
                self.diagnostics.mcse_mean,
                self.diagnostics.mcse_sd,
                self.diagnostics.minimum_ebfmi,
                self.posterior_predictive.replicated_total_mean,
                self.posterior_predictive.replicated_total_sd,
                self.posterior_predictive.replicated_zero_cells_mean,
            ])
            || self.posterior_predictive.replicated_total_mean < 0.0
            || self.posterior_predictive.replicated_total_sd < 0.0
            || self.posterior_predictive.replicated_zero_cells_mean < 0.0
            || self.posterior_predictive.replicated_zero_cells_mean > self.cells.len() as f64
        {
            return Err(BayesError::WorkerContract(
                "inhomogeneous Poisson diagnostics or predictive summaries are invalid".into(),
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
                "inhomogeneous Poisson fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: InhomogeneousPoissonFitWorkerRequest,
        input: InhomogeneousPoissonFitInputIdentity,
    ) -> InhomogeneousPoissonFitResult {
        InhomogeneousPoissonFitResult {
            format: "marklab.bayesian_inhomogeneous_poisson_fit",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            window: request.window,
            grid_x: request.grid_x,
            grid_y: request.grid_y,
            cell_width_um: request.cell_width_um,
            cell_height_um: request.cell_height_um,
            cell_area_um2: request.cell_area_um2,
            observed_event_count: request.events.len(),
            sampling: self.sampling,
            fit_state: self.fit_state,
            claim_status: if self.fit_state == FitState::Complete {
                "experimental"
            } else {
                "diagnostic_only_nonconverged"
            },
            posterior: self.posterior,
            cells: self.cells,
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
        || (positive && (summary.mean <= 0.0 || summary.interval_lower < 0.0))
    {
        return Err(BayesError::WorkerContract(
            "inhomogeneous Poisson posterior summary is invalid".into(),
        ));
    }
    Ok(())
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12 * left.abs().max(right.abs()).max(1.0)
}

#[derive(Debug, Serialize)]
pub struct InhomogeneousPoissonFitInputIdentity {
    pub events_path: String,
    pub quadrature_path: String,
    pub events_sha256: String,
    pub quadrature_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct InhomogeneousPoissonFitResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: InhomogeneousPoissonFitModelIr,
    pub input: InhomogeneousPoissonFitInputIdentity,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub cell_width_um: f64,
    pub cell_height_um: f64,
    pub cell_area_um2: f64,
    pub observed_event_count: usize,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub posterior: InhomogeneousPoissonPosterior,
    pub cells: Vec<InhomogeneousPoissonCellSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: InhomogeneousPoissonPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_rejects_constant_quadrature_covariate() {
        let events = (0..20)
            .map(|index| InhomogeneousPoissonEvent {
                event_id: format!("e{index:02}"),
                x_um: (index % 4) as f64 + 0.25,
                y_um: 0.5,
                covariate: 0.0,
                offset: 0.0,
            })
            .collect();
        let quadrature = (0..4)
            .map(|ix| MidpointQuadratureValue {
                ix,
                iy: 0,
                covariate: 0.0,
                offset: 0.0,
            })
            .collect();
        assert!(matches!(
            InhomogeneousPoissonFitWorkerRequest::new(
                InhomogeneousPoissonFitSpec {
                    window: RectangularWindow {
                        xmin_um: 0.0,
                        ymin_um: 0.0,
                        xmax_um: 4.0,
                        ymax_um: 1.0,
                    },
                    grid_x: 4,
                    grid_y: 1,
                    events,
                    quadrature,
                    intercept_prior_mean: 0.0,
                    intercept_prior_sd: 2.0,
                    coefficient_prior_mean: 0.0,
                    coefficient_prior_sd: 2.0,
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
            ),
            Err(BayesError::InvalidSpec(_))
        ));
    }
}

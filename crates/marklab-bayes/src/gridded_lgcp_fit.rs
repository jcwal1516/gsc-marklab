use serde::{Deserialize, Serialize};

use crate::{
    build_gridded_lgcp,
    model::{
        BackendContract, DiagnosticPolicy, PYMC_VERSION, WORKER_REQUEST_FORMAT,
        WORKER_REQUEST_VERSION,
    },
    sha256_hex, BayesError, FitState, GriddedLgcpCell, GriddedLgcpModelIr, GriddedLgcpSpec,
    NormalMeanDiagnostics, NutsSamplingSpec, RectangularWindow, SamplingSummary, SarScalarSummary,
    WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct GriddedLgcpFitSpec {
    pub construction: GriddedLgcpSpec,
}

#[derive(Clone, Debug, Serialize)]
pub struct GriddedLgcpFitModelIr {
    pub construction: GriddedLgcpModelIr,
    pub field_parameterization: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct GriddedLgcpFitResourceLimits {
    pub maximum_cells: u32,
    pub maximum_total_iterations: u64,
    pub maximum_draw_cell_work: u64,
    pub maximum_output_bytes: u64,
    pub maximum_predictive_replicates: u32,
    pub maximum_predictive_points: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GriddedLgcpPredictionControls {
    pub replicates: u32,
    pub seed: u64,
    pub maximum_total_points: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GriddedLgcpFitWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: GriddedLgcpFitModelIr,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub event_count: usize,
    pub cell_area_um2: f64,
    pub cells: Vec<GriddedLgcpCell>,
    pub field_covariance: Vec<f64>,
    pub field_cholesky: Vec<f64>,
    pub covariance_sha256: String,
    pub sampling: NutsSamplingSpec,
    pub prediction: GriddedLgcpPredictionControls,
    pub resources: GriddedLgcpFitResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl GriddedLgcpFitWorkerRequest {
    pub fn new(
        spec: GriddedLgcpFitSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "gridded LGCP timeout must be in 1-3600 seconds".into(),
            ));
        }
        let built = build_gridded_lgcp(spec.construction)
            .map_err(|error| BayesError::InvalidSpec(error.to_string()))?;
        let dimension = built.cells.len();
        if !(4..=36).contains(&dimension) {
            return Err(BayesError::InvalidSpec(
                "fitted gridded LGCP requires 4-36 cells".into(),
            ));
        }
        let covariate_min = built
            .cells
            .iter()
            .map(|cell| cell.covariate)
            .fold(f64::INFINITY, f64::min);
        let covariate_max = built
            .cells
            .iter()
            .map(|cell| cell.covariate)
            .fold(f64::NEG_INFINITY, f64::max);
        if covariate_max - covariate_min
            <= f64::EPSILON.sqrt() * covariate_min.abs().max(covariate_max.abs()).max(1.0)
        {
            return Err(BayesError::InvalidSpec(
                "fitted gridded LGCP covariate must vary materially".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        let iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        if iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "gridded LGCP NUTS iterations exceed 400000".into(),
            ));
        }
        let maximum_draw_cell_work = 1_000_000;
        let completed_draws = u64::from(sampling.chains) * u64::from(sampling.draws_per_chain);
        if completed_draws * dimension as u64 > maximum_draw_cell_work {
            return Err(BayesError::InvalidSpec(
                "gridded LGCP posterior cell work exceeds 1000000".into(),
            ));
        }
        let covariance_sha256 = sha256_hex(&serde_json::to_vec(&built.field_covariance)?);
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
            model: GriddedLgcpFitModelIr {
                construction: built.model,
                field_parameterization: "noncentered_fixed_cholesky_times_standard_normal",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            window: built.window,
            grid_x: built.grid_x,
            grid_y: built.grid_y,
            event_count: built.event_count,
            cell_area_um2: built.cell_area_um2,
            cells: built.cells,
            field_covariance: built.field_covariance,
            field_cholesky: built.field_cholesky,
            covariance_sha256,
            sampling,
            prediction: GriddedLgcpPredictionControls {
                replicates: 0,
                seed: 0,
                maximum_total_points: 100_000,
            },
            resources: GriddedLgcpFitResourceLimits {
                maximum_cells: 36,
                maximum_total_iterations,
                maximum_draw_cell_work,
                maximum_output_bytes: 2 * 1_048_576,
                maximum_predictive_replicates: 32,
                maximum_predictive_points: 100_000,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }

    pub fn with_prediction(
        mut self,
        replicates: u32,
        seed: u64,
        maximum_total_points: u64,
    ) -> Result<Self, BayesError> {
        if !(1..=self.resources.maximum_predictive_replicates).contains(&replicates)
            || !(1..=self.resources.maximum_predictive_points).contains(&maximum_total_points)
        {
            return Err(BayesError::InvalidSpec(
                "gridded LGCP prediction requires 1-32 replicates and a 1-100000 point cap".into(),
            ));
        }
        self.prediction = GriddedLgcpPredictionControls {
            replicates,
            seed,
            maximum_total_points,
        };
        self.resources.maximum_output_bytes = 16 * 1_048_576;
        Ok(self)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpPosterior {
    pub intercept: SarScalarSummary,
    pub coefficient: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpCellPosterior {
    pub ix: u32,
    pub iy: u32,
    pub count: u64,
    pub latent_effect: SarScalarSummary,
    pub intensity: SarScalarSummary,
    pub expected_count: SarScalarSummary,
    pub pearson_residual: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpPosteriorPredictive {
    pub observed_total_count: u64,
    pub observed_zero_cells: u64,
    pub replicated_total_mean: f64,
    pub replicated_total_sd: f64,
    pub replicated_zero_cells_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpPredictiveDraw {
    pub chain: u32,
    pub draw: u32,
    pub intercept: f64,
    pub coefficient: f64,
    pub latent_effect: Vec<f64>,
    pub intensity: Vec<f64>,
    pub expected_count: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpPredictivePoint {
    pub point_id: String,
    pub ix: u32,
    pub iy: u32,
    pub x_um: f64,
    pub y_um: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpPredictivePattern {
    pub replicate: u32,
    pub posterior_draw: GriddedLgcpPredictiveDraw,
    pub cell_counts: Vec<u64>,
    pub total_count: u64,
    pub points: Vec<GriddedLgcpPredictivePoint>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpFitWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GriddedLgcpPosterior,
    pub cells: Vec<GriddedLgcpCellPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: GriddedLgcpPosteriorPredictive,
    pub patterns: Vec<GriddedLgcpPredictivePattern>,
}

impl GriddedLgcpFitWorkerResult {
    pub fn validate(
        &self,
        request: &GriddedLgcpFitWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly
            || self.format != "marklab.pymc_gridded_lgcp_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.cells.len() != request.cells.len()
        {
            return Err(BayesError::WorkerContract(
                "gridded LGCP result identity or dimensions mismatch".into(),
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
                "gridded LGCP sampling counts mismatch".into(),
            ));
        }
        validate_scalar(&self.posterior.intercept, false)?;
        validate_scalar(&self.posterior.coefficient, false)?;
        for (actual, expected) in self.cells.iter().zip(&request.cells) {
            if actual.ix != expected.ix
                || actual.iy != expected.iy
                || actual.count != expected.count
            {
                return Err(BayesError::WorkerContract(
                    "gridded LGCP cell identity mismatch".into(),
                ));
            }
            validate_scalar(&actual.latent_effect, false)?;
            validate_scalar(&actual.intensity, true)?;
            validate_scalar(&actual.expected_count, true)?;
            validate_scalar(&actual.pearson_residual, false)?;
        }
        let observed_total = request.cells.iter().map(|cell| cell.count).sum::<u64>();
        let observed_zeros = request.cells.iter().filter(|cell| cell.count == 0).count() as u64;
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
            || !(0.0..=self.cells.len() as f64)
                .contains(&self.posterior_predictive.replicated_zero_cells_mean)
        {
            return Err(BayesError::WorkerContract(
                "gridded LGCP diagnostics or predictive summaries are invalid".into(),
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
                "gridded LGCP fit state disagrees with diagnostics".into(),
            ));
        }
        self.validate_patterns(request)?;
        Ok(())
    }

    fn validate_patterns(&self, request: &GriddedLgcpFitWorkerRequest) -> Result<(), BayesError> {
        let expected_patterns = if self.fit_state == FitState::Complete {
            request.prediction.replicates as usize
        } else {
            0
        };
        if self.patterns.len() != expected_patterns {
            return Err(BayesError::WorkerContract(
                "gridded LGCP predictive pattern count disagrees with fit state or request".into(),
            ));
        }
        let dimension = request.cells.len();
        let total_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        let cell_width =
            (request.window.xmax_um - request.window.xmin_um) / f64::from(request.grid_x);
        let cell_height =
            (request.window.ymax_um - request.window.ymin_um) / f64::from(request.grid_y);
        let mut total_points = 0_u64;
        for (replicate_index, pattern) in self.patterns.iter().enumerate() {
            if pattern.replicate as usize >= expected_patterns
                || pattern.replicate as usize != replicate_index
                || pattern.cell_counts.len() != dimension
                || pattern.posterior_draw.latent_effect.len() != dimension
                || pattern.posterior_draw.intensity.len() != dimension
                || pattern.posterior_draw.expected_count.len() != dimension
            {
                return Err(BayesError::WorkerContract(
                    "gridded LGCP predictive pattern dimensions or order are invalid".into(),
                ));
            }
            let flattened = u64::from(pattern.replicate) * total_draws
                / u64::from(request.prediction.replicates);
            if pattern.posterior_draw.chain
                != (flattened / u64::from(request.sampling.draws_per_chain)) as u32
                || pattern.posterior_draw.draw
                    != (flattened % u64::from(request.sampling.draws_per_chain)) as u32
                || !finite(&[
                    pattern.posterior_draw.intercept,
                    pattern.posterior_draw.coefficient,
                ])
            {
                return Err(BayesError::WorkerContract(
                    "gridded LGCP predictive source draw identity is invalid".into(),
                ));
            }
            for (index, cell) in request.cells.iter().enumerate() {
                let latent = pattern.posterior_draw.latent_effect[index];
                let intensity = pattern.posterior_draw.intensity[index];
                let expected = pattern.posterior_draw.expected_count[index];
                let recomputed_intensity = (pattern.posterior_draw.intercept
                    + pattern.posterior_draw.coefficient * cell.covariate
                    + cell.offset
                    + latent)
                    .exp();
                let recomputed_expected = cell.area_um2 * recomputed_intensity;
                if !finite(&[
                    latent,
                    intensity,
                    expected,
                    recomputed_intensity,
                    recomputed_expected,
                ]) || intensity <= 0.0
                    || expected <= 0.0
                    || !close(intensity, recomputed_intensity)
                    || !close(expected, recomputed_expected)
                {
                    return Err(BayesError::WorkerContract(
                        "gridded LGCP predictive draw intensity is invalid".into(),
                    ));
                }
            }
            let count_sum = pattern.cell_counts.iter().try_fold(0_u64, |sum, value| {
                sum.checked_add(*value).ok_or_else(|| {
                    BayesError::WorkerContract(
                        "gridded LGCP predictive cell counts overflow".into(),
                    )
                })
            })?;
            if count_sum != pattern.total_count
                || pattern.points.len() as u64 != pattern.total_count
            {
                return Err(BayesError::WorkerContract(
                    "gridded LGCP predictive point totals mismatch".into(),
                ));
            }
            let mut point_index = 0_usize;
            for (cell_index, count) in pattern.cell_counts.iter().copied().enumerate() {
                let cell = &request.cells[cell_index];
                let xmin = request.window.xmin_um + f64::from(cell.ix) * cell_width;
                let ymin = request.window.ymin_um + f64::from(cell.iy) * cell_height;
                for local_index in 0..count {
                    let point = &pattern.points[point_index];
                    if point.point_id
                        != format!(
                            "rep:{}:cell:{cell_index}:point:{local_index}",
                            pattern.replicate
                        )
                        || point.ix != cell.ix
                        || point.iy != cell.iy
                        || !finite(&[point.x_um, point.y_um])
                        || !(xmin..xmin + cell_width).contains(&point.x_um)
                        || !(ymin..ymin + cell_height).contains(&point.y_um)
                    {
                        return Err(BayesError::WorkerContract(
                            "gridded LGCP predictive point identity or coordinate is invalid"
                                .into(),
                        ));
                    }
                    point_index += 1;
                }
            }
            total_points = total_points
                .checked_add(pattern.total_count)
                .ok_or_else(|| {
                    BayesError::WorkerContract("gridded LGCP predictive points overflow".into())
                })?;
        }
        if total_points > request.prediction.maximum_total_points {
            return Err(BayesError::WorkerContract(
                "gridded LGCP predictive point cap exceeded".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: GriddedLgcpFitWorkerRequest,
        input: GriddedLgcpFitInputIdentity,
    ) -> GriddedLgcpFitResult {
        GriddedLgcpFitResult {
            format: "marklab.bayesian_gridded_lgcp_fit",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            window: request.window,
            grid_x: request.grid_x,
            grid_y: request.grid_y,
            cell_area_um2: request.cell_area_um2,
            covariance_sha256: request.covariance_sha256,
            observed_event_count: request.event_count,
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

    pub fn into_prediction(
        self,
        request: GriddedLgcpFitWorkerRequest,
        input: GriddedLgcpFitInputIdentity,
    ) -> Result<GriddedLgcpPredictionResult, BayesError> {
        if self.fit_state != FitState::Complete || request.prediction.replicates == 0 {
            return Err(BayesError::WorkerContract(
                "gridded LGCP prediction requires a complete fit and positive replicate count"
                    .into(),
            ));
        }
        let patterns = self.patterns;
        let total_points = patterns.iter().map(|pattern| pattern.total_count).sum();
        Ok(GriddedLgcpPredictionResult {
            format: "marklab.bayesian_gridded_lgcp_posterior_predictive",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            window: request.window,
            grid_x: request.grid_x,
            grid_y: request.grid_y,
            cell_area_um2: request.cell_area_um2,
            covariance_sha256: request.covariance_sha256,
            observed_event_count: request.event_count,
            sampling: self.sampling,
            fit_state: self.fit_state,
            claim_status: "experimental_discretized_prediction",
            diagnostics: self.diagnostics,
            prediction_seed: request.prediction.seed,
            replicate_count: request.prediction.replicates,
            total_points,
            patterns,
            approximation: GriddedLgcpPredictionApproximation {
                intensity_rule: "piecewise_constant_at_exact_full_cell",
                location_rule: "uniform_within_exact_full_cell",
                continuous_intensity_bound: "not_available",
                thinning: "not_used",
                maximum_cell_width_um: (request.window.xmax_um - request.window.xmin_um)
                    / f64::from(request.grid_x),
                maximum_cell_height_um: (request.window.ymax_um - request.window.ymin_um)
                    / f64::from(request.grid_y),
            },
            request_sha256: self.request_sha256,
        })
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
            "gridded LGCP posterior summary is invalid".into(),
        ));
    }
    Ok(())
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-10 * actual.abs().max(expected.abs()).max(1.0)
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpFitInputIdentity {
    pub events_path: String,
    pub grid_path: String,
    pub events_sha256: String,
    pub grid_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpFitResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: GriddedLgcpFitModelIr,
    pub input: GriddedLgcpFitInputIdentity,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub cell_area_um2: f64,
    pub covariance_sha256: String,
    pub observed_event_count: usize,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub posterior: GriddedLgcpPosterior,
    pub cells: Vec<GriddedLgcpCellPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: GriddedLgcpPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpPredictionApproximation {
    pub intensity_rule: &'static str,
    pub location_rule: &'static str,
    pub continuous_intensity_bound: &'static str,
    pub thinning: &'static str,
    pub maximum_cell_width_um: f64,
    pub maximum_cell_height_um: f64,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpPredictionResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: GriddedLgcpFitModelIr,
    pub input: GriddedLgcpFitInputIdentity,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub cell_area_um2: f64,
    pub covariance_sha256: String,
    pub observed_event_count: usize,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub diagnostics: NormalMeanDiagnostics,
    pub prediction_seed: u64,
    pub replicate_count: u32,
    pub total_points: u64,
    pub patterns: Vec<GriddedLgcpPredictivePattern>,
    pub approximation: GriddedLgcpPredictionApproximation,
    pub request_sha256: String,
}

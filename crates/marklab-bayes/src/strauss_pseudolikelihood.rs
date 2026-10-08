use serde::{Deserialize, Serialize};

use crate::{
    model::{BackendContract, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    BayesError, FitState, RectangularWindow, StraussPoint, WorkerBackend,
};

const SCIPY_BACKEND_VERSION: &str = "scipy-1.18.1";

#[derive(Clone, Debug)]
pub struct StraussPseudolikelihoodSpec {
    pub points: Vec<StraussPoint>,
    pub window: RectangularWindow,
    pub interaction_radius_um: f64,
    pub coarse_grid_x: u32,
    pub coarse_grid_y: u32,
    pub fine_grid_x: u32,
    pub fine_grid_y: u32,
    pub beta_min_per_um2: f64,
    pub beta_max_per_um2: f64,
    pub gamma_min: f64,
    pub gamma_max: f64,
    pub maximum_iterations: u32,
    pub maximum_neighbor_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct StraussPseudolikelihoodModelIr {
    pub family: &'static str,
    pub conditional_intensity: &'static str,
    pub quadrature: &'static str,
    pub pair_boundary: &'static str,
    pub uncertainty: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StraussPseudolikelihoodBounds {
    pub beta_min_per_um2: f64,
    pub beta_max_per_um2: f64,
    pub gamma_min: f64,
    pub gamma_max: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct StraussPseudolikelihoodRow {
    pub node_id: String,
    pub node_kind: &'static str,
    pub x_um: f64,
    pub y_um: f64,
    pub weight_um2: f64,
    pub response: f64,
    pub neighbor_count: u32,
    pub block: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct StraussPseudolikelihoodResolution {
    pub grid_x: u32,
    pub grid_y: u32,
    pub cell_area_um2: f64,
    pub weight_sum_um2: f64,
    pub observed_nodes: u32,
    pub dummy_nodes: u32,
    pub neighbor_visits: u64,
    pub rows: Vec<StraussPseudolikelihoodRow>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StraussPseudolikelihoodResources {
    pub maximum_table_rows: u32,
    pub maximum_iterations: u32,
    pub maximum_neighbor_visits: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct StraussPseudolikelihoodWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: StraussPseudolikelihoodModelIr,
    pub window: RectangularWindow,
    pub interaction_radius_um: f64,
    pub bounds: StraussPseudolikelihoodBounds,
    pub maximum_iterations: u32,
    pub coarse: StraussPseudolikelihoodResolution,
    pub fine: StraussPseudolikelihoodResolution,
    pub resources: StraussPseudolikelihoodResources,
}

impl StraussPseudolikelihoodWorkerRequest {
    pub fn new(
        mut spec: StraussPseudolikelihoodSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        validate_spec(&spec, timeout_seconds)?;
        spec.points.sort_by(|a, b| a.point_id.cmp(&b.point_id));
        let coarse = build_resolution(
            &spec.points,
            &spec.window,
            spec.interaction_radius_um,
            spec.coarse_grid_x,
            spec.coarse_grid_y,
            spec.maximum_neighbor_visits,
        )?;
        let remaining_visits = spec
            .maximum_neighbor_visits
            .checked_sub(coarse.neighbor_visits)
            .ok_or_else(|| BayesError::InvalidSpec("neighbor visit budget exhausted".into()))?;
        let fine = build_resolution(
            &spec.points,
            &spec.window,
            spec.interaction_radius_um,
            spec.fine_grid_x,
            spec.fine_grid_y,
            remaining_visits,
        )?;
        if coarse.rows.len() + fine.rows.len() > 100_000 {
            return Err(BayesError::InvalidSpec(
                "Strauss pseudolikelihood tables exceed 100000 rows".into(),
            ));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_BACKEND_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: StraussPseudolikelihoodModelIr {
                family: "strauss_inhibitory_point_process",
                conditional_intensity: "beta_times_gamma_power_inclusive_radius_neighbor_count",
                quadrature: "berman_turner_observed_plus_one_midpoint_dummy_per_cell",
                pair_boundary: "euclidean_distance_less_than_or_equal_radius",
                uncertainty: "model_hessian_and_two_by_two_window_block_sandwich",
                backend_capability: "bounded_weighted_poisson_optimization",
                maturity: "experimental_pseudolikelihood",
            },
            window: spec.window,
            interaction_radius_um: spec.interaction_radius_um,
            bounds: StraussPseudolikelihoodBounds {
                beta_min_per_um2: spec.beta_min_per_um2,
                beta_max_per_um2: spec.beta_max_per_um2,
                gamma_min: spec.gamma_min,
                gamma_max: spec.gamma_max,
            },
            maximum_iterations: spec.maximum_iterations,
            coarse,
            fine,
            resources: StraussPseudolikelihoodResources {
                maximum_table_rows: 100_000,
                maximum_iterations: 100_000,
                maximum_neighbor_visits: spec.maximum_neighbor_visits,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
        })
    }
}

fn validate_spec(spec: &StraussPseudolikelihoodSpec, timeout: u64) -> Result<(), BayesError> {
    if spec.points.is_empty()
        || spec.points.len() > 10_000
        || ![
            spec.window.xmin_um,
            spec.window.ymin_um,
            spec.window.xmax_um,
            spec.window.ymax_um,
            spec.interaction_radius_um,
            spec.beta_min_per_um2,
            spec.beta_max_per_um2,
            spec.gamma_min,
            spec.gamma_max,
        ]
        .into_iter()
        .all(f64::is_finite)
        || spec.window.xmin_um >= spec.window.xmax_um
        || spec.window.ymin_um >= spec.window.ymax_um
        || spec.interaction_radius_um <= 0.0
        || spec.beta_min_per_um2 <= 0.0
        || spec.beta_min_per_um2 >= spec.beta_max_per_um2
        || spec.gamma_min <= 0.0
        || spec.gamma_min >= spec.gamma_max
        || spec.gamma_max > 1.0
        || spec.coarse_grid_x == 0
        || spec.coarse_grid_y == 0
        || !spec.fine_grid_x.is_multiple_of(spec.coarse_grid_x)
        || !spec.fine_grid_y.is_multiple_of(spec.coarse_grid_y)
        || !(10..=100_000).contains(&spec.maximum_iterations)
        || !(1..=100_000_000).contains(&spec.maximum_neighbor_visits)
        || !(1..=3_600).contains(&timeout)
    {
        return Err(BayesError::InvalidSpec(
            "Strauss pseudolikelihood inputs, nested grids, bounds, or resources are invalid"
                .into(),
        ));
    }
    let mut ids = std::collections::HashSet::new();
    for point in &spec.points {
        if point.point_id.is_empty()
            || point.point_id.trim() != point.point_id
            || !point.x_um.is_finite()
            || !point.y_um.is_finite()
            || !ids.insert(point.point_id.as_str())
            || !(spec.window.xmin_um..spec.window.xmax_um).contains(&point.x_um)
            || !(spec.window.ymin_um..spec.window.ymax_um).contains(&point.y_um)
        {
            return Err(BayesError::InvalidSpec(
                "Strauss pseudolikelihood points require unique IDs inside the exact window".into(),
            ));
        }
    }
    Ok(())
}

fn build_resolution(
    points: &[StraussPoint],
    window: &RectangularWindow,
    radius: f64,
    grid_x: u32,
    grid_y: u32,
    visit_cap: u64,
) -> Result<StraussPseudolikelihoodResolution, BayesError> {
    let cells = u64::from(grid_x) * u64::from(grid_y);
    if cells > 100_000 {
        return Err(BayesError::InvalidSpec(
            "pseudolikelihood grid is too large".into(),
        ));
    }
    let width = (window.xmax_um - window.xmin_um) / f64::from(grid_x);
    let height = (window.ymax_um - window.ymin_um) / f64::from(grid_y);
    let area = width * height;
    let mut cell_counts = vec![0_u32; cells as usize];
    for point in points {
        let ix = ((point.x_um - window.xmin_um) / width).floor() as usize;
        let iy = ((point.y_um - window.ymin_um) / height).floor() as usize;
        cell_counts[iy * grid_x as usize + ix] += 1;
    }
    let row_count = points.len() as u64 + cells;
    let visits = row_count
        .checked_mul(points.len() as u64)
        .ok_or_else(|| BayesError::InvalidSpec("neighbor work overflows".into()))?;
    if visits > visit_cap {
        return Err(BayesError::InvalidSpec(
            "neighbor visit cap exceeded".into(),
        ));
    }
    let mut rows = Vec::with_capacity(row_count as usize);
    for point in points {
        let ix = ((point.x_um - window.xmin_um) / width).floor() as usize;
        let iy = ((point.y_um - window.ymin_um) / height).floor() as usize;
        let weight = area / f64::from(cell_counts[iy * grid_x as usize + ix] + 1);
        rows.push(make_row(
            format!("event:{}", point.point_id),
            "observed",
            point.x_um,
            point.y_um,
            weight,
            1.0 / weight,
            points,
            Some(point.point_id.as_str()),
            radius,
            window,
        )?);
    }
    for iy in 0..grid_y {
        for ix in 0..grid_x {
            let index = iy as usize * grid_x as usize + ix as usize;
            let weight = area / f64::from(cell_counts[index] + 1);
            rows.push(make_row(
                format!("dummy:{ix}:{iy}"),
                "dummy",
                window.xmin_um + (f64::from(ix) + 0.5) * width,
                window.ymin_um + (f64::from(iy) + 0.5) * height,
                weight,
                0.0,
                points,
                None,
                radius,
                window,
            )?);
        }
    }
    let weight_sum = rows.iter().map(|row| row.weight_um2).sum::<f64>();
    Ok(StraussPseudolikelihoodResolution {
        grid_x,
        grid_y,
        cell_area_um2: area,
        weight_sum_um2: weight_sum,
        observed_nodes: points.len() as u32,
        dummy_nodes: cells as u32,
        neighbor_visits: visits,
        rows,
    })
}

#[allow(clippy::too_many_arguments)]
fn make_row(
    node_id: String,
    node_kind: &'static str,
    x: f64,
    y: f64,
    weight: f64,
    response: f64,
    points: &[StraussPoint],
    excluded_id: Option<&str>,
    radius: f64,
    window: &RectangularWindow,
) -> Result<StraussPseudolikelihoodRow, BayesError> {
    let mut neighbors = 0_u32;
    for point in points {
        if excluded_id == Some(point.point_id.as_str()) {
            continue;
        }
        let distance = (x - point.x_um).hypot(y - point.y_um);
        if !distance.is_finite() {
            return Err(BayesError::InvalidSpec(
                "neighbor distance is non-finite".into(),
            ));
        }
        if distance <= radius {
            neighbors += 1;
        }
    }
    let block_x = u32::from(x >= (window.xmin_um + window.xmax_um) / 2.0);
    let block_y = u32::from(y >= (window.ymin_um + window.ymax_um) / 2.0);
    Ok(StraussPseudolikelihoodRow {
        node_id,
        node_kind,
        x_um: x,
        y_um: y,
        weight_um2: weight,
        response,
        neighbor_count: neighbors,
        block: block_y * 2 + block_x,
    })
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StraussPseudolikelihoodFitSummary {
    pub grid_x: u32,
    pub grid_y: u32,
    pub beta_per_um2: f64,
    pub gamma: f64,
    pub log_beta: f64,
    pub log_gamma: f64,
    pub objective: f64,
    pub initial_objective: f64,
    pub gradient_norm: f64,
    pub hessian_condition: f64,
    pub model_se_log_beta: f64,
    pub model_se_log_gamma: f64,
    pub robust_se_log_beta: f64,
    pub robust_se_log_gamma: f64,
    pub weight_sum_um2: f64,
    pub converged: bool,
    pub evaluations: u64,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StraussPseudolikelihoodRefinement {
    pub absolute_beta_difference: f64,
    pub absolute_gamma_difference: f64,
    pub absolute_objective_difference: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StraussPseudolikelihoodWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub coarse_fit: StraussPseudolikelihoodFitSummary,
    pub fine_fit: StraussPseudolikelihoodFitSummary,
    pub refinement: StraussPseudolikelihoodRefinement,
}

impl StraussPseudolikelihoodWorkerResult {
    pub fn validate(
        &self,
        request: &StraussPseudolikelihoodWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.scipy_strauss_pseudolikelihood_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_BACKEND_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "Strauss pseudolikelihood result identity mismatch".into(),
            ));
        }
        validate_fit(&self.coarse_fit, &request.coarse, &request.bounds)?;
        validate_fit(&self.fine_fit, &request.fine, &request.bounds)?;
        if !close(
            self.refinement.absolute_beta_difference,
            (self.coarse_fit.beta_per_um2 - self.fine_fit.beta_per_um2).abs(),
        ) || !close(
            self.refinement.absolute_gamma_difference,
            (self.coarse_fit.gamma - self.fine_fit.gamma).abs(),
        ) || !close(
            self.refinement.absolute_objective_difference,
            (self.coarse_fit.objective - self.fine_fit.objective).abs(),
        ) {
            return Err(BayesError::WorkerContract(
                "Strauss pseudolikelihood refinement arithmetic mismatch".into(),
            ));
        }
        let complete = self.coarse_fit.converged
            && self.fine_fit.converged
            && self.coarse_fit.gradient_norm <= 1e-6
            && self.fine_fit.gradient_norm <= 1e-6;
        if (self.fit_state == FitState::Complete) != complete {
            return Err(BayesError::WorkerContract(
                "Strauss pseudolikelihood fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: StraussPseudolikelihoodWorkerRequest,
        input: StraussPseudolikelihoodInputIdentity,
    ) -> StraussPseudolikelihoodResult {
        StraussPseudolikelihoodResult {
            format: "marklab.strauss_pseudolikelihood_fit",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            window: request.window,
            interaction_radius_um: request.interaction_radius_um,
            bounds: request.bounds,
            fit_state: self.fit_state,
            claim_status: if self.fit_state == FitState::Complete {
                "experimental_pseudolikelihood"
            } else {
                "diagnostic_only_nonconverged"
            },
            coarse_fit: self.coarse_fit,
            fine_fit: self.fine_fit,
            refinement: self.refinement,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_fit(
    fit: &StraussPseudolikelihoodFitSummary,
    resolution: &StraussPseudolikelihoodResolution,
    bounds: &StraussPseudolikelihoodBounds,
) -> Result<(), BayesError> {
    if fit.grid_x != resolution.grid_x
        || fit.grid_y != resolution.grid_y
        || ![
            fit.beta_per_um2,
            fit.gamma,
            fit.log_beta,
            fit.log_gamma,
            fit.objective,
            fit.initial_objective,
            fit.gradient_norm,
            fit.hessian_condition,
            fit.model_se_log_beta,
            fit.model_se_log_gamma,
            fit.robust_se_log_beta,
            fit.robust_se_log_gamma,
            fit.weight_sum_um2,
        ]
        .into_iter()
        .all(f64::is_finite)
        || !(bounds.beta_min_per_um2..=bounds.beta_max_per_um2).contains(&fit.beta_per_um2)
        || !(bounds.gamma_min..=bounds.gamma_max).contains(&fit.gamma)
        || !close(fit.log_beta, fit.beta_per_um2.ln())
        || !close(fit.log_gamma, fit.gamma.ln())
        || fit.objective >= fit.initial_objective
        || fit.gradient_norm < 0.0
        || fit.hessian_condition < 1.0
        || fit.model_se_log_beta <= 0.0
        || fit.model_se_log_gamma <= 0.0
        || fit.robust_se_log_beta <= 0.0
        || fit.robust_se_log_gamma <= 0.0
        || fit.evaluations == 0
        || fit.message.is_empty()
        || !close(fit.weight_sum_um2, resolution.weight_sum_um2)
        || !close(
            fit.objective,
            objective(&resolution.rows, fit.log_beta, fit.log_gamma),
        )
    {
        return Err(BayesError::WorkerContract(
            "Strauss pseudolikelihood fit summary is invalid".into(),
        ));
    }
    Ok(())
}

fn objective(rows: &[StraussPseudolikelihoodRow], log_beta: f64, log_gamma: f64) -> f64 {
    rows.iter()
        .map(|row| {
            let eta = log_beta + f64::from(row.neighbor_count) * log_gamma;
            row.weight_um2 * (eta.exp() - row.response * eta)
        })
        .sum()
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-8 * actual.abs().max(expected.abs()).max(1.0)
}

#[derive(Debug, Serialize)]
pub struct StraussPseudolikelihoodInputIdentity {
    pub path: String,
    pub sha256: String,
    pub point_count: u32,
}

#[derive(Debug, Serialize)]
pub struct StraussPseudolikelihoodResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: StraussPseudolikelihoodModelIr,
    pub input: StraussPseudolikelihoodInputIdentity,
    pub window: RectangularWindow,
    pub interaction_radius_um: f64,
    pub bounds: StraussPseudolikelihoodBounds,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub coarse_fit: StraussPseudolikelihoodFitSummary,
    pub fine_fit: StraussPseudolikelihoodFitSummary,
    pub refinement: StraussPseudolikelihoodRefinement,
    pub request_sha256: String,
}

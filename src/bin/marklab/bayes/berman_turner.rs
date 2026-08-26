use std::path::PathBuf;

use marklab_bayes::{
    berman_turner_refinement, sha256_hex, BermanTurnerError, BermanTurnerNode,
    BermanTurnerRefinementSpec, BermanTurnerResolution, RectangularWindow,
};
use serde::Serialize;

use super::{
    inhomogeneous_poisson::{read_events, read_quadrature},
    publish_json, BayesCliError,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    events_path: PathBuf,
    coarse_quadrature_path: PathBuf,
    fine_quadrature_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    coarse_grid_x: u32,
    coarse_grid_y: u32,
    fine_grid_x: u32,
    fine_grid_y: u32,
    intercept: f64,
    coefficient: f64,
    convergence_tolerance: f64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let events = read_events(&events_path)?;
    let coarse_quadrature = read_quadrature(&coarse_quadrature_path)?;
    let fine_quadrature = read_quadrature(&fine_quadrature_path)?;
    let events_sha256 = sha256_hex(&serde_json::to_vec(&events)?);
    let coarse_quadrature_sha256 = sha256_hex(&serde_json::to_vec(&coarse_quadrature)?);
    let fine_quadrature_sha256 = sha256_hex(&serde_json::to_vec(&fine_quadrature)?);
    let result = berman_turner_refinement(BermanTurnerRefinementSpec {
        window: RectangularWindow {
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
        },
        events,
        coarse_grid_x,
        coarse_grid_y,
        coarse_quadrature,
        fine_grid_x,
        fine_grid_y,
        fine_quadrature,
        intercept,
        coefficient,
        convergence_tolerance,
    })
    .map_err(map_error)?;
    publish_json(
        &output_path,
        &BermanTurnerOutput {
            format: "marklab.berman_turner_refinement",
            version: 1,
            events_path,
            coarse_quadrature_path,
            fine_quadrature_path,
            events_sha256,
            coarse_quadrature_sha256,
            fine_quadrature_sha256,
            coordinate_unit: "micrometer",
            area_unit: "square_micrometer",
            window: RectangularWindow {
                xmin_um,
                ymin_um,
                xmax_um,
                ymax_um,
            },
            window_area_um2: result.window_area_um2,
            intercept,
            coefficient,
            coarse: resolution(&result.coarse),
            fine: resolution(&result.fine),
            absolute_objective_change: result.absolute_objective_change,
            convergence_tolerance: result.convergence_tolerance,
            converged: result.converged,
            fine_table: result.fine.table,
            interpretation: "berman_turner_weighted_poisson_objective_without_constants",
            claim_status: "experimental_approximation_diagnostic",
        },
    )
}

fn resolution(value: &BermanTurnerResolution) -> ResolutionOutput {
    ResolutionOutput {
        grid_x: value.grid_x,
        grid_y: value.grid_y,
        cell_area_um2: value.cell_area_um2,
        node_count: value.node_count,
        observed_node_count: value.observed_node_count,
        dummy_node_count: value.dummy_node_count,
        weight_sum_um2: value.weight_sum_um2,
        weighted_objective: value.weighted_objective,
    }
}

fn map_error(error: BermanTurnerError) -> BayesCliError {
    match error {
        BermanTurnerError::InvalidInput(message) => BayesCliError::Input(message),
        BermanTurnerError::Numerical(message) => BayesCliError::Backend(message),
    }
}

#[derive(Serialize)]
struct ResolutionOutput {
    grid_x: u32,
    grid_y: u32,
    cell_area_um2: f64,
    node_count: usize,
    observed_node_count: usize,
    dummy_node_count: usize,
    weight_sum_um2: f64,
    weighted_objective: f64,
}

#[derive(Serialize)]
struct BermanTurnerOutput {
    format: &'static str,
    version: u32,
    events_path: PathBuf,
    coarse_quadrature_path: PathBuf,
    fine_quadrature_path: PathBuf,
    events_sha256: String,
    coarse_quadrature_sha256: String,
    fine_quadrature_sha256: String,
    coordinate_unit: &'static str,
    area_unit: &'static str,
    window: RectangularWindow,
    window_area_um2: f64,
    intercept: f64,
    coefficient: f64,
    coarse: ResolutionOutput,
    fine: ResolutionOutput,
    absolute_objective_change: f64,
    convergence_tolerance: f64,
    converged: bool,
    fine_table: Vec<BermanTurnerNode>,
    interpretation: &'static str,
    claim_status: &'static str,
}

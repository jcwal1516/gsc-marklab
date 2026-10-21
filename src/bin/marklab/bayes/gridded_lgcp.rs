use std::path::PathBuf;

use marklab_bayes::{
    build_gridded_lgcp, json_sha256, GriddedLgcpError, GriddedLgcpModelIr, GriddedLgcpSpec,
    RectangularWindow,
};
use serde::Serialize;

use super::cli_schema::BuildGriddedLgcpArgs;
use super::{
    inhomogeneous_poisson::{read_events, read_quadrature},
    publish_json, CliError,
};

pub(super) fn run(args: BuildGriddedLgcpArgs) -> Result<(), CliError> {
    let BuildGriddedLgcpArgs {
        events,
        grid,
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        grid_x,
        grid_y,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        out,
    } = args;
    let events_path = events;
    let grid_path = grid;
    let output_path = out;
    let events = read_events(&events_path)?;
    let grid = read_quadrature(&grid_path)?;
    let events_sha256 = json_sha256(&events)?;
    let grid_sha256 = json_sha256(&grid)?;
    let result = build_gridded_lgcp(GriddedLgcpSpec {
        window: RectangularWindow {
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
        },
        grid_x,
        grid_y,
        events,
        grid,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
    })
    .map_err(map_error)?;
    let covariance_sha256 = json_sha256(&result.field_covariance)?;
    publish_json(
        &output_path,
        &GriddedLgcpOutput {
            format: "marklab.gridded_lgcp_model",
            version: 1,
            events_path,
            grid_path,
            events_sha256,
            grid_sha256,
            model: result.model,
            window: result.window,
            grid_x: result.grid_x,
            grid_y: result.grid_y,
            event_count: result.event_count,
            cell_area_um2: result.cell_area_um2,
            cells: result.cells,
            field_covariance: result.field_covariance,
            covariance_sha256,
            covariance_positive_definite: result.covariance_positive_definite,
            dense_covariance_elements: result.dense_covariance_elements,
            dense_factorization_work_units: result.dense_factorization_work_units,
            fit_state: "not_fitted",
            claim_status: "experimental_model_construction",
        },
    )
}

fn map_error(error: GriddedLgcpError) -> CliError {
    match error {
        GriddedLgcpError::InvalidInput(message) => CliError::Input(message),
        GriddedLgcpError::Numerical(message) => CliError::Backend(message),
    }
}

#[derive(Serialize)]
struct GriddedLgcpOutput {
    format: &'static str,
    version: u32,
    events_path: PathBuf,
    grid_path: PathBuf,
    events_sha256: String,
    grid_sha256: String,
    model: GriddedLgcpModelIr,
    window: RectangularWindow,
    grid_x: u32,
    grid_y: u32,
    event_count: usize,
    cell_area_um2: f64,
    cells: Vec<marklab_bayes::GriddedLgcpCell>,
    field_covariance: Vec<f64>,
    covariance_sha256: String,
    covariance_positive_definite: bool,
    dense_covariance_elements: usize,
    dense_factorization_work_units: u64,
    fit_state: &'static str,
    claim_status: &'static str,
}

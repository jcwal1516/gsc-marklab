use std::{fs, path::PathBuf};

use marklab_simulation::{
    simulate_vascular_transport, SimulationError, UptakeCell, VascularTransportSpec, VesselSource,
};
use serde::Deserialize;

use crate::{MarklabError, Result};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    grid_x: u32,
    grid_y: u32,
    spacing_x_um: f64,
    spacing_y_um: f64,
    initial_concentration_row_major: Vec<f64>,
    diffusion_um2_per_time_row_major: Vec<f64>,
    velocity_x_um_per_time_row_major: Vec<f64>,
    velocity_y_um_per_time_row_major: Vec<f64>,
    flow_approximation: String,
    vessel_sources: Vec<VesselInput>,
    uptake_cells: Vec<UptakeInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VesselInput {
    vessel_id: String,
    x_um: f64,
    y_um: f64,
    source_concentration_per_time: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UptakeInput {
    cell_id: String,
    x_um: f64,
    y_um: f64,
    linear_uptake_per_time: f64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    input: PathBuf,
    final_time: f64,
    time_step: f64,
    hypoxia_threshold: f64,
    record_every_steps: u32,
    maximum_cell_steps: u64,
    out: PathBuf,
) -> Result<()> {
    let metadata = fs::metadata(&input).map_err(|source| MarklabError::io(&input, source))?;
    if metadata.len() > 16 * 1024 * 1024 {
        return Err(MarklabError::Validation(
            "vascular transport input exceeds 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input).map_err(|source| MarklabError::io(&input, source))?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result = simulate_vascular_transport(VascularTransportSpec {
        grid_x: input.grid_x,
        grid_y: input.grid_y,
        spacing_x_um: input.spacing_x_um,
        spacing_y_um: input.spacing_y_um,
        initial_concentration_row_major: input.initial_concentration_row_major,
        diffusion_um2_per_time_row_major: input.diffusion_um2_per_time_row_major,
        velocity_x_um_per_time_row_major: input.velocity_x_um_per_time_row_major,
        velocity_y_um_per_time_row_major: input.velocity_y_um_per_time_row_major,
        flow_approximation: input.flow_approximation,
        vessel_sources: input
            .vessel_sources
            .into_iter()
            .map(|source| VesselSource {
                vessel_id: source.vessel_id,
                x_um: source.x_um,
                y_um: source.y_um,
                source_concentration_per_time: source.source_concentration_per_time,
            })
            .collect(),
        uptake_cells: input
            .uptake_cells
            .into_iter()
            .map(|cell| UptakeCell {
                cell_id: cell.cell_id,
                x_um: cell.x_um,
                y_um: cell.y_um,
                linear_uptake_per_time: cell.linear_uptake_per_time,
            })
            .collect(),
        final_time,
        time_step,
        hypoxia_threshold,
        record_every_steps,
        maximum_cell_steps,
    })
    .map_err(map)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|source| MarklabError::io(parent, source))?;
    }
    fs::write(&out, serde_json::to_vec_pretty(&result)?)
        .map_err(|source| MarklabError::io(&out, source))
}

fn map(error: SimulationError) -> MarklabError {
    MarklabError::Validation(error.to_string())
}

use std::{fs, path::PathBuf};

use marklab_simulation::{evolve_interface_level_set, LevelSetSpec, SimulationError};
use serde::Deserialize;

use crate::{MarklabError, Result};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    grid_x: u32,
    grid_y: u32,
    spacing_x_um: f64,
    spacing_y_um: f64,
    initial_phi_row_major: Vec<f64>,
    normal_speed_um_per_time_row_major: Vec<f64>,
    curvature_weight_um2_per_time: f64,
}

/// Arguments of `marklab simulate evolve-interface`.
#[derive(clap::Args, Debug)]
pub struct EvolveInterfaceArgs {
    /// JSON containing the grid, initial level-set field, normal speeds, and curvature weight.
    #[arg(long)]
    input: PathBuf,
    /// Time at which to stop evolving the interface.
    #[arg(long)]
    final_time: f64,
    /// Time increment for each solver step.
    #[arg(long)]
    time_step: f64,
    /// Steps between signed-distance reinitializations; zero disables reinitialization.
    #[arg(long)]
    reinitialize_every_steps: u32,
    /// Number of solver steps between recorded snapshots.
    #[arg(long)]
    record_every_steps: u32,
    /// Maximum number of grid-cell time-step updates.
    #[arg(long)]
    maximum_cell_steps: u64,
    /// Maximum grid-point to interface-sample distance evaluations across reinitializations.
    #[arg(long)]
    maximum_reinitialization_distance_visits: u64,
    /// File to write the result to.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) fn run(args: EvolveInterfaceArgs) -> Result<()> {
    let EvolveInterfaceArgs {
        input,
        final_time,
        time_step,
        reinitialize_every_steps,
        record_every_steps,
        maximum_cell_steps,
        maximum_reinitialization_distance_visits,
        out,
    } = args;
    let bytes = super::read_input(&input, "level-set")?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result = evolve_interface_level_set(LevelSetSpec {
        grid_x: input.grid_x,
        grid_y: input.grid_y,
        spacing_x_um: input.spacing_x_um,
        spacing_y_um: input.spacing_y_um,
        initial_phi_row_major: input.initial_phi_row_major,
        normal_speed_um_per_time_row_major: input.normal_speed_um_per_time_row_major,
        curvature_weight_um2_per_time: input.curvature_weight_um2_per_time,
        final_time,
        time_step,
        reinitialize_every_steps,
        record_every_steps,
        maximum_cell_steps,
        maximum_reinitialization_distance_visits,
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

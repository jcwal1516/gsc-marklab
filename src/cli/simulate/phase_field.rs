use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use marklab_simulation::{simulate_phase_field, PhaseFieldSpec};
use serde::Deserialize;

use crate::{MarklabError, Result};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    grid_x: u32,
    grid_y: u32,
    spacing_x_um: f64,
    spacing_y_um: f64,
    initial_phase_row_major: Vec<f64>,
    mobility_um2_per_time: f64,
    bulk_energy_scale: f64,
    gradient_energy_coefficient_um2: f64,
}

/// Arguments of `marklab simulate phase-field`.
#[derive(clap::Args, Debug)]
pub struct PhaseFieldArgs {
    /// JSON grid, initial phase field, mobility, and double-well energy parameters.
    #[arg(long)]
    input: PathBuf,
    /// Time at which to stop the simulation.
    #[arg(long)]
    final_time: f64,
    /// Maximum time increment for each solver step.
    #[arg(long)]
    time_step: f64,
    /// Number of solver steps between retained summaries.
    #[arg(long)]
    record_every_steps: u32,
    /// Maximum number of grid-cell time-step updates.
    #[arg(long)]
    maximum_cell_steps: u64,
    /// File to write the result to.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) fn run(args: PhaseFieldArgs) -> Result<()> {
    let PhaseFieldArgs {
        input,
        final_time,
        time_step,
        record_every_steps,
        maximum_cell_steps,
        out,
    } = args;
    let bytes = super::read_input(&input, "phase-field")?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result = simulate_phase_field(PhaseFieldSpec {
        grid_x: input.grid_x,
        grid_y: input.grid_y,
        spacing_x_um: input.spacing_x_um,
        spacing_y_um: input.spacing_y_um,
        initial_phase_row_major: input.initial_phase_row_major,
        mobility_um2_per_time: input.mobility_um2_per_time,
        bulk_energy_scale: input.bulk_energy_scale,
        gradient_energy_coefficient_um2: input.gradient_energy_coefficient_um2,
        final_time,
        time_step,
        record_every_steps,
        maximum_cell_steps,
    })
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|source| MarklabError::io(parent, source))?;
    }
    let bytes = serde_json::to_vec_pretty(&result)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&out)
        .map_err(|source| MarklabError::io(&out, source))?;
    output
        .write_all(&bytes)
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.sync_all())
        .map_err(|source| MarklabError::io(&out, source))
}

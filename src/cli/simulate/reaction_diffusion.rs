use std::{fs, path::PathBuf};

use marklab_simulation::{
    simulate_reaction_diffusion, ReactionDiffusionSpec, ReactionModel, SimulationError,
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
    initial_row_major: Vec<f64>,
    diffusion_um2_per_time: f64,
    reaction: ReactionModel,
}

/// Arguments of `marklab simulate reaction-diffusion`.
#[derive(clap::Args, Debug)]
pub struct ReactionDiffusionArgs {
    /// JSON grid with initial_row_major field, diffusion_um2_per_time and reaction model.
    #[arg(long)]
    input: PathBuf,
    /// Simulation end time in the model's time units.
    #[arg(long)]
    final_time: f64,
    /// Simulation time step in the model's time units.
    #[arg(long)]
    time_step: f64,
    /// Number of simulation steps between retained state summaries.
    #[arg(long)]
    record_every_steps: u32,
    /// Upper bound on cell steps.
    #[arg(long)]
    maximum_cell_steps: u64,
    /// File to write the result to.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) fn run(args: ReactionDiffusionArgs) -> Result<()> {
    let ReactionDiffusionArgs {
        input,
        final_time,
        time_step,
        record_every_steps,
        maximum_cell_steps,
        out,
    } = args;
    let bytes = super::read_input(&input, "reaction-diffusion")?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result = simulate_reaction_diffusion(ReactionDiffusionSpec {
        grid_x: input.grid_x,
        grid_y: input.grid_y,
        spacing_x_um: input.spacing_x_um,
        spacing_y_um: input.spacing_y_um,
        initial_row_major: input.initial_row_major,
        diffusion_um2_per_time: input.diffusion_um2_per_time,
        reaction: input.reaction,
        final_time,
        time_step,
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

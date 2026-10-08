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

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    input: PathBuf,
    final_time: f64,
    time_step: f64,
    record_every_steps: u32,
    maximum_cell_steps: u64,
    out: PathBuf,
) -> Result<()> {
    let metadata = fs::metadata(&input).map_err(|source| MarklabError::io(&input, source))?;
    if metadata.len() > 16 * 1024 * 1024 {
        return Err(MarklabError::Validation(
            "reaction-diffusion input exceeds 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input).map_err(|source| MarklabError::io(&input, source))?;
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

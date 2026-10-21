use std::fs;

use marklab_sbi::{smc_abc_growth_front, SmcAbcGrowthFrontSpec};
use marklab_simulation::GrowthFrontInitialPoint;
use serde::Deserialize;

use super::cli_schema::SmcAbcGrowthFrontArgs;
use super::{publish_json, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    initial: Vec<InitialPoint>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialPoint {
    position_um: f64,
    density: f64,
}

pub(super) fn run(args: SmcAbcGrowthFrontArgs) -> Result<(), CliError> {
    let SmcAbcGrowthFrontArgs {
        input,
        diffusion_um2_per_time,
        carrying_capacity,
        final_time,
        time_step,
        front_threshold_fraction,
        observed_final_mass,
        mass_scale,
        growth_rate_prior_min,
        growth_rate_prior_max,
        epsilon_schedule,
        particles,
        maximum_proposals_per_stage,
        maximum_cell_steps_per_proposal,
        seed,
        out,
    } = args;
    let input_path = input;
    let output_path = out;
    let metadata = fs::metadata(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "SMC-ABC input must be a regular file within 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path,
        source,
    })?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result = smc_abc_growth_front(SmcAbcGrowthFrontSpec {
        initial: input
            .initial
            .into_iter()
            .map(|row| GrowthFrontInitialPoint {
                position_um: row.position_um,
                density: row.density,
            })
            .collect(),
        diffusion_um2_per_time,
        carrying_capacity,
        final_time,
        time_step,
        front_threshold_fraction,
        observed_final_mass,
        mass_scale,
        growth_rate_prior_min,
        growth_rate_prior_max,
        epsilon_schedule,
        particles,
        maximum_proposals_per_stage,
        maximum_cell_steps_per_proposal,
        seed,
    })
    .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

use std::{fs, path::PathBuf};

use marklab_sbi::{smc_abc_growth_front, SmcAbcGrowthFrontSpec};
use marklab_simulation::GrowthFrontInitialPoint;
use serde::Deserialize;

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

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

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    diffusion_um2_per_time: f64,
    carrying_capacity: f64,
    final_time: f64,
    time_step: f64,
    front_threshold_fraction: f64,
    observed_final_mass: f64,
    mass_scale: f64,
    growth_rate_prior_min: f64,
    growth_rate_prior_max: f64,
    epsilon_schedule: Vec<f64>,
    particles: u32,
    maximum_proposals_per_stage: u32,
    maximum_cell_steps_per_proposal: u64,
    seed: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "SMC-ABC input must be a regular file within 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
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
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

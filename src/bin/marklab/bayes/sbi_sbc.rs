use std::{fs, path::PathBuf};

use marklab_sbi::{growth_front_rejection_abc_sbc, GrowthFrontRejectionAbcSbcSpec};
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
    mass_scale: f64,
    growth_rate_prior_min: f64,
    growth_rate_prior_max: f64,
    epsilon: f64,
    posterior_draws: u32,
    maximum_proposals_per_replicate: u32,
    replicates: u32,
    coverage_probability: f64,
    maximum_cell_steps_per_simulation: u64,
    seed: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "SBI SBC input must be a regular file within 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path,
        source,
    })?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result = growth_front_rejection_abc_sbc(GrowthFrontRejectionAbcSbcSpec {
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
        mass_scale,
        growth_rate_prior_min,
        growth_rate_prior_max,
        epsilon,
        posterior_draws,
        maximum_proposals_per_replicate,
        replicates,
        coverage_probability,
        maximum_cell_steps_per_simulation,
        seed,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

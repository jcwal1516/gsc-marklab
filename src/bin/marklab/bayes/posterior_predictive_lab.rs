use std::fs;

use marklab_sbi::{
    growth_front_posterior_predictive_laboratory, GrowthFrontPosteriorPredictiveLabSpec,
};
use marklab_simulation::GrowthFrontInitialPoint;
use serde::Deserialize;

use super::cli_schema::GrowthFrontPosteriorPredictiveLabArgs;
use super::{publish_json, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    initial: Vec<InitialPoint>,
    posterior_growth_rate_draws: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialPoint {
    position_um: f64,
    density: f64,
}

pub(super) fn run(args: GrowthFrontPosteriorPredictiveLabArgs) -> Result<(), CliError> {
    let GrowthFrontPosteriorPredictiveLabArgs {
        input,
        diffusion_um2_per_time,
        carrying_capacity,
        final_time,
        time_step,
        front_threshold_fraction,
        observed_final_mass,
        observed_maximum_density,
        replicates,
        interval_probability,
        discrepancy_alpha,
        maximum_cell_steps_per_replicate,
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
            "posterior-predictive input must be a regular file within 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path,
        source,
    })?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result =
        growth_front_posterior_predictive_laboratory(GrowthFrontPosteriorPredictiveLabSpec {
            initial: input
                .initial
                .into_iter()
                .map(|row| GrowthFrontInitialPoint {
                    position_um: row.position_um,
                    density: row.density,
                })
                .collect(),
            posterior_growth_rate_draws: input.posterior_growth_rate_draws,
            diffusion_um2_per_time,
            carrying_capacity,
            final_time,
            time_step,
            front_threshold_fraction,
            observed_final_mass,
            observed_maximum_density,
            replicates,
            interval_probability,
            discrepancy_alpha,
            maximum_cell_steps_per_replicate,
            seed,
        })
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

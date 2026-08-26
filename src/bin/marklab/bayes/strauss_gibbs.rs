use std::path::PathBuf;

use marklab_bayes::{
    simulate_strauss_birth_death, RectangularWindow, StraussBirthDeathError, StraussBirthDeathSpec,
};

use super::{publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    beta_per_um2: f64,
    gamma: f64,
    interaction_radius_um: f64,
    iterations: u32,
    burn_in: u32,
    seed: u64,
    maximum_points: u32,
    maximum_neighbor_visits: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let result = simulate_strauss_birth_death(StraussBirthDeathSpec {
        window: RectangularWindow {
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
        },
        beta_per_um2,
        gamma,
        interaction_radius_um,
        iterations,
        burn_in,
        seed,
        maximum_points,
        maximum_neighbor_visits,
    })
    .map_err(map_error)?;
    publish_json(&output_path, &result)
}

fn map_error(error: StraussBirthDeathError) -> BayesCliError {
    match error {
        StraussBirthDeathError::InvalidSpec(message)
        | StraussBirthDeathError::Resource(message) => BayesCliError::Input(message),
        StraussBirthDeathError::Numerical(message) => BayesCliError::Backend(message),
    }
}

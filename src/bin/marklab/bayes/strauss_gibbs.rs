use marklab_bayes::{
    simulate_strauss_birth_death, RectangularWindow, StraussBirthDeathError, StraussBirthDeathSpec,
};

use super::cli_schema::SimulateStraussBirthDeathArgs;
use super::{publish_json, CliError};

pub(super) fn run(args: SimulateStraussBirthDeathArgs) -> Result<(), CliError> {
    let SimulateStraussBirthDeathArgs {
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        beta_per_um2,
        gamma,
        interaction_radius_um,
        iterations,
        burn_in,
        seed,
        maximum_points,
        maximum_neighbor_visits,
        out,
    } = args;
    let output_path = out;
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

fn map_error(error: StraussBirthDeathError) -> CliError {
    match error {
        StraussBirthDeathError::InvalidSpec(message)
        | StraussBirthDeathError::Resource(message) => CliError::Input(message),
        StraussBirthDeathError::Numerical(message) => CliError::Backend(message),
    }
}

use marklab_bayes::RectangularWindow;
use marklab_simulation::{simulate_thomas_process, ThomasProcessError, ThomasProcessSpec};

use super::cli_schema::SimulateThomasProcessArgs;
use super::{publish_json, CliError};

pub(super) fn run(args: SimulateThomasProcessArgs) -> Result<(), CliError> {
    let SimulateThomasProcessArgs {
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        kappa_parent_per_um2,
        mu_offspring,
        sigma_um,
        seed,
        maximum_parents,
        maximum_offspring,
        out,
    } = args;
    let output_path = out;
    let result = simulate_thomas_process(ThomasProcessSpec {
        window: RectangularWindow {
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
        },
        kappa_parent_per_um2,
        mu_offspring,
        sigma_um,
        seed,
        maximum_parents,
        maximum_offspring,
    })
    .map_err(map_error)?;
    publish_json(&output_path, &result)
}

fn map_error(error: ThomasProcessError) -> CliError {
    match error {
        ThomasProcessError::InvalidSpec(message) | ThomasProcessError::Resource(message) => {
            CliError::Input(message)
        }
        ThomasProcessError::Numerical(message) => CliError::Backend(message),
    }
}

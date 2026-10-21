use marklab_bayes::RectangularWindow;
use marklab_simulation::{
    simulate_matern_cluster_process, MaternClusterProcessError, MaternClusterProcessSpec,
};

use super::cli_schema::SimulateMaternClusterProcessArgs;
use super::{publish_json, CliError};

pub(super) fn run(args: SimulateMaternClusterProcessArgs) -> Result<(), CliError> {
    let SimulateMaternClusterProcessArgs {
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        kappa_parent_per_um2,
        mu_offspring,
        radius_um,
        seed,
        maximum_parents,
        maximum_offspring,
        out,
    } = args;
    let output_path = out;
    let result = simulate_matern_cluster_process(MaternClusterProcessSpec {
        window: RectangularWindow {
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
        },
        kappa_parent_per_um2,
        mu_offspring,
        radius_um,
        seed,
        maximum_parents,
        maximum_offspring,
    })
    .map_err(map_error)?;
    publish_json(&output_path, &result)
}

fn map_error(error: MaternClusterProcessError) -> CliError {
    match error {
        MaternClusterProcessError::InvalidSpec(message)
        | MaternClusterProcessError::Resource(message) => CliError::Input(message),
        MaternClusterProcessError::Numerical(message) => CliError::Backend(message),
    }
}

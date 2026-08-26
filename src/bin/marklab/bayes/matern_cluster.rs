use std::path::PathBuf;

use marklab_bayes::{
    simulate_matern_cluster_process, MaternClusterProcessError, MaternClusterProcessSpec,
    RectangularWindow,
};

use super::{publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    kappa_parent_per_um2: f64,
    mu_offspring: f64,
    radius_um: f64,
    seed: u64,
    maximum_parents: u64,
    maximum_offspring: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
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

fn map_error(error: MaternClusterProcessError) -> BayesCliError {
    match error {
        MaternClusterProcessError::InvalidSpec(message)
        | MaternClusterProcessError::Resource(message) => BayesCliError::Input(message),
        MaternClusterProcessError::Numerical(message) => BayesCliError::Backend(message),
    }
}

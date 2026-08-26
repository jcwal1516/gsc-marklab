use std::path::PathBuf;

use marklab_bayes::{
    simulate_thomas_process, RectangularWindow, ThomasProcessError, ThomasProcessSpec,
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
    sigma_um: f64,
    seed: u64,
    maximum_parents: u64,
    maximum_offspring: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
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

fn map_error(error: ThomasProcessError) -> BayesCliError {
    match error {
        ThomasProcessError::InvalidSpec(message) | ThomasProcessError::Resource(message) => {
            BayesCliError::Input(message)
        }
        ThomasProcessError::Numerical(message) => BayesCliError::Backend(message),
    }
}

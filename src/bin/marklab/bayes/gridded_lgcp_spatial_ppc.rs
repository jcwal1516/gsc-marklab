use std::path::PathBuf;

use marklab_bayes::{gridded_lgcp_spatial_ppc, NutsSamplingSpec};

use super::{gridded_lgcp_fit, publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    events_path: PathBuf,
    grid_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    grid_x: u32,
    grid_y: u32,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    replicates: u32,
    prediction_seed: u64,
    maximum_total_points: u64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = gridded_lgcp_fit::prepare(
        events_path,
        grid_path,
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        grid_x,
        grid_y,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        sampling,
        timeout_seconds,
        Some((replicates, prediction_seed, maximum_total_points)),
    )?;
    let observed_counts = prepared
        .request
        .cells
        .iter()
        .map(|cell| cell.count)
        .collect::<Vec<_>>();
    let result = gridded_lgcp_fit::execute(&prepared)?;
    let prediction = result.into_prediction(prepared.request, prepared.input_identity)?;
    publish_json(
        &output_path,
        &gridded_lgcp_spatial_ppc(prediction, &observed_counts)?,
    )
}

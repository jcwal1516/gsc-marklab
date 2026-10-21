use marklab_bayes::gridded_lgcp_spatial_ppc;

use super::cli_schema::GriddedLgcpSpatialPpcArgs;
use super::{gridded_lgcp_fit, publish_json, CliError};

pub(super) fn run(args: GriddedLgcpSpatialPpcArgs) -> Result<(), CliError> {
    let GriddedLgcpSpatialPpcArgs {
        model,
        replicates,
        prediction_seed,
        maximum_total_points,
        out,
    } = args;
    let output_path = out;
    let prepared = gridded_lgcp_fit::prepare(
        Some((replicates, prediction_seed, maximum_total_points)),
        model,
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

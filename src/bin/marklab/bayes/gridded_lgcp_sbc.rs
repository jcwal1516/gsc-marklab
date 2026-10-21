use marklab_bayes::NumpyroGriddedLgcpSbcWorkerRequest;

use super::cli_schema::GriddedLgcpSbcArgs;
use super::{cross_backend, gridded_lgcp_fit, CliError};

pub(super) fn run(args: GriddedLgcpSbcArgs) -> Result<(), CliError> {
    let GriddedLgcpSbcArgs {
        model,
        replicates,
        minimum_rank_uniformity_p_value,
        minimum_coverage_90,
        maximum_coverage_90,
        out,
    } = args;
    let output_path = out;
    let prepared = gridded_lgcp_fit::prepare(None, model)?;
    let timeout_seconds = prepared.timeout_seconds;
    cross_backend::sbc(timeout_seconds, &output_path, |identity| {
        Ok(NumpyroGriddedLgcpSbcWorkerRequest::new(
            prepared.request,
            replicates,
            minimum_rank_uniformity_p_value,
            minimum_coverage_90,
            maximum_coverage_90,
            identity.environment_lock_sha256,
            identity.worker_sha256,
            timeout_seconds,
        )?)
    })
}

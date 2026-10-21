use marklab_bayes::{NumpyroBetaBinomialSbcWorkerRequest, SbcControls};

use super::cli_schema::BetaBinomialHierarchySbcArgs;
use super::{beta_binomial_hierarchy, cross_backend, CliError};

pub(super) fn run(args: BetaBinomialHierarchySbcArgs) -> Result<(), CliError> {
    let BetaBinomialHierarchySbcArgs {
        model,
        replicates,
        minimum_rank_uniformity_p_value,
        minimum_coverage_90,
        maximum_coverage_90,
        out,
    } = args;
    let output_path = out;
    let prepared = beta_binomial_hierarchy::prepare(model)?;
    let timeout_seconds = prepared.timeout_seconds;
    cross_backend::sbc(timeout_seconds, &output_path, |identity| {
        Ok(NumpyroBetaBinomialSbcWorkerRequest::new(
            prepared.request,
            SbcControls {
                replicates,
                minimum_rank_uniformity_p_value,
                minimum_coverage: minimum_coverage_90,
                maximum_coverage: maximum_coverage_90,
            },
            identity.environment_lock_sha256,
            identity.worker_sha256,
            timeout_seconds,
        )?)
    })
}

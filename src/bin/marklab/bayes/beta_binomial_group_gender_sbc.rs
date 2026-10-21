use marklab_bayes::{NumpyroBetaBinomialGroupGenderSbcWorkerRequest, SbcControls};

use super::cli_schema::BetaBinomialGroupGenderRegressionSbcArgs;
use super::{beta_binomial_group_gender_regression, cross_backend, CliError};

pub(super) fn run(args: BetaBinomialGroupGenderRegressionSbcArgs) -> Result<(), CliError> {
    let BetaBinomialGroupGenderRegressionSbcArgs {
        model,
        replicates,
        minimum_rank_uniformity_p_value,
        minimum_coverage_90,
        maximum_coverage_90,
        out,
    } = args;
    let output_path = out;
    let prepared = beta_binomial_group_gender_regression::prepare(model)?;
    let timeout_seconds = prepared.timeout_seconds;
    cross_backend::sbc(timeout_seconds, &output_path, |identity| {
        Ok(NumpyroBetaBinomialGroupGenderSbcWorkerRequest::new(
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

use marklab_bayes::{NumpyroDirichletMultinomialGroupSbcWorkerRequest, SbcControls};

use super::cli_schema::DirichletMultinomialGroupSbcArgs;
use super::{cross_backend, dirichlet_multinomial_group, CliError};

pub(super) fn run(args: DirichletMultinomialGroupSbcArgs) -> Result<(), CliError> {
    let DirichletMultinomialGroupSbcArgs {
        model,
        replicates,
        minimum_rank_uniformity_p_value,
        minimum_coverage_90,
        maximum_coverage_90,
        out: output_path,
    } = args;
    let prepared = dirichlet_multinomial_group::prepare(model)?;
    let timeout_seconds = prepared.timeout_seconds;
    cross_backend::sbc(timeout_seconds, &output_path, |identity| {
        Ok(NumpyroDirichletMultinomialGroupSbcWorkerRequest::new(
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

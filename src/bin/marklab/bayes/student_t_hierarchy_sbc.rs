use marklab_bayes::{NumpyroStudentTSbcWorkerRequest, SbcControls};

use super::cli_schema::StudentTHierarchySbcArgs;
use super::{cross_backend, student_t_hierarchy, CliError};

pub(super) fn run(args: StudentTHierarchySbcArgs) -> Result<(), CliError> {
    let StudentTHierarchySbcArgs {
        model,
        replicates,
        minimum_rank_uniformity_p_value,
        minimum_coverage_90,
        maximum_coverage_90,
        out,
    } = args;
    let output_path = out;
    let prepared = student_t_hierarchy::prepare(model)?;
    let timeout_seconds = prepared.timeout_seconds;
    cross_backend::sbc(timeout_seconds, &output_path, |identity| {
        Ok(NumpyroStudentTSbcWorkerRequest::new(
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

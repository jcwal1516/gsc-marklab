use marklab_bayes::NumpyroHierarchySbcWorkerRequest;

use super::cli_schema::{HierarchicalNormalModelArgs, HierarchicalNormalSbcArgs};
use super::{cross_backend, CliError};

pub(super) fn run(args: HierarchicalNormalSbcArgs) -> Result<(), CliError> {
    let HierarchicalNormalSbcArgs {
        model:
            HierarchicalNormalModelArgs {
                global_prior_mean,
                global_prior_sd,
                between_patient_sd_prior,
                known_sigma,
                timeout_seconds,
                nuts,
            },
        patients,
        observations_per_patient,
        replicates,
        minimum_rank_uniformity_p_value,
        minimum_coverage_90,
        maximum_coverage_90,
        out,
    } = args;
    let sampling = nuts.spec();
    let output_path = out;
    cross_backend::sbc(timeout_seconds, &output_path, |identity| {
        Ok(NumpyroHierarchySbcWorkerRequest::new(
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            known_sigma,
            patients,
            observations_per_patient,
            replicates,
            sampling,
            minimum_rank_uniformity_p_value,
            minimum_coverage_90,
            maximum_coverage_90,
            identity.environment_lock_sha256,
            identity.worker_sha256,
            timeout_seconds,
        )?)
    })
}

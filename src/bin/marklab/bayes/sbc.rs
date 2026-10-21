use marklab_bayes::{
    sha256_hex, NormalMeanSbcSpec, NormalMeanSbcWorkerRequest, NormalMeanSbcWorkerResult,
};

use super::cli_schema::NormalMeanSbcArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError};

pub(super) fn run(args: NormalMeanSbcArgs) -> Result<(), CliError> {
    let NormalMeanSbcArgs {
        prior_mean,
        prior_sd,
        known_sigma,
        observations_per_replicate,
        replicates,
        posterior_draws,
        seed,
        timeout_seconds,
        out,
    } = args;
    let output_path = out;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_numpy_scipy_sbc_worker.py")?;
    let request = NormalMeanSbcWorkerRequest::new(
        NormalMeanSbcSpec {
            prior_mean,
            prior_sd,
            known_sigma,
            observations_per_replicate,
            replicates,
            posterior_draws,
            seed,
        },
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_numpy_scipy_sbc_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NormalMeanSbcWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request))
}

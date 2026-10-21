use marklab_bayes::{
    sha256_hex, NormalMeanSmcInputIdentity, NormalMeanSmcWorkerRequest, NormalMeanSmcWorkerResult,
    NormalMeanSpec, SmcSamplingSpec,
};

use super::cli_schema::NormalMeanSmcArgs;
use super::worker_identity;
use super::{observations_digest, publish_json, read_observations, run_worker, CliError};

pub(super) fn run(args: NormalMeanSmcArgs) -> Result<(), CliError> {
    let NormalMeanSmcArgs {
        input,
        prior_mean,
        prior_sd,
        known_sigma,
        particles,
        chains,
        ess_target,
        correlation_threshold,
        seed,
        timeout_seconds,
        out,
    } = args;
    let input_path = input;
    let sampling = SmcSamplingSpec {
        particles,
        chains,
        ess_target,
        correlation_threshold,
        seed,
    };
    let output_path = out;
    let observations = read_observations(&input_path)?;
    let input = NormalMeanSmcInputIdentity {
        path: input_path.display().to_string(),
        observation_count: observations.len(),
        observations_sha256: observations_digest(&observations),
    };
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_smc_worker.py")?;
    let request = NormalMeanSmcWorkerRequest::new(
        NormalMeanSpec {
            prior_mean,
            prior_sd,
            known_sigma,
            observations,
        },
        sampling,
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_smc_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NormalMeanSmcWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, input))
}

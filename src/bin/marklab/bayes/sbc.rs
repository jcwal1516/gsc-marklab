use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NormalMeanSbcSpec, NormalMeanSbcWorkerRequest, NormalMeanSbcWorkerResult,
};

use super::{publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    prior_mean: f64,
    prior_sd: f64,
    known_sigma: f64,
    observations_per_replicate: u32,
    replicates: u32,
    posterior_draws: u32,
    seed: u64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_numpy_scipy_sbc_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
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
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
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

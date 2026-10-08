use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NormalMeanSmcInputIdentity, NormalMeanSmcWorkerRequest, NormalMeanSmcWorkerResult,
    NormalMeanSpec, SmcSamplingSpec,
};

use super::{observations_digest, publish_json, read_observations, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    prior_mean: f64,
    prior_sd: f64,
    known_sigma: f64,
    sampling: SmcSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let observations = read_observations(&input_path)?;
    let input = NormalMeanSmcInputIdentity {
        path: input_path.display().to_string(),
        observation_count: observations.len(),
        observations_sha256: observations_digest(&observations),
    };
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_smc_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NormalMeanSmcWorkerRequest::new(
        NormalMeanSpec {
            prior_mean,
            prior_sd,
            known_sigma,
            observations,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
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

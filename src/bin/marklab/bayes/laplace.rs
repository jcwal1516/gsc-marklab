use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, PoissonExposureObservation, PoissonLaplaceInputIdentity, PoissonLaplaceSpec,
    PoissonLaplaceWorkerRequest, PoissonLaplaceWorkerResult,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    observation_id: String,
    count: u64,
    exposure: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    prior_mean: f64,
    prior_sd: f64,
    initial_log_rate: f64,
    max_iterations: u32,
    gradient_tolerance: f64,
    seed: u64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let observations = read_observations(&input_path)?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_scipy_pytensor_laplace_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = PoissonLaplaceWorkerRequest::new(
        PoissonLaplaceSpec {
            prior_mean,
            prior_sd,
            initial_log_rate,
            max_iterations,
            gradient_tolerance,
            observations,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
        seed,
    )?;
    let identity = PoissonLaplaceInputIdentity {
        path: input_path.display().to_string(),
        observation_count: request.observations.len(),
        observations_sha256: sha256_hex(&serde_json::to_vec(&request.observations)?),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_scipy_pytensor_laplace_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: PoissonLaplaceWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, identity))
}

fn read_observations(
    path: &std::path::Path,
) -> Result<Vec<PoissonExposureObservation>, BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "Poisson Laplace input must be a regular file within the 16 MiB limit",
    )?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["observation_id", "count", "exposure"])
    {
        return Err(BayesCliError::Input(
            "Poisson Laplace headers must be exactly: observation_id,count,exposure".into(),
        ));
    }
    let mut observations = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        let row = row?;
        observations.push(PoissonExposureObservation {
            observation_id: row.observation_id,
            count: row.count,
            exposure: row.exposure,
        });
        if observations.len() > 100_000 {
            return Err(BayesCliError::Input(
                "Poisson Laplace observation count exceeds 100000".into(),
            ));
        }
    }
    Ok(observations)
}

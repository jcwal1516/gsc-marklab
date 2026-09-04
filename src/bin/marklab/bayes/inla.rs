use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NutsSamplingSpec, PoissonExposureObservation, PoissonInlaInputIdentity,
    PoissonInlaSpec, PoissonInlaWorkerRequest, PoissonInlaWorkerResult,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    region_id: String,
    count: u64,
    exposure: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    latent_mean: f64,
    tau_shape: f64,
    tau_rate: f64,
    log_tau_min: f64,
    log_tau_max: f64,
    grid_points: u32,
    endpoint_mass_limit: f64,
    sampling: NutsSamplingSpec,
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
    let worker_path = worker_directory.join("marklab_scipy_pytensor_pymc_inla_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = PoissonInlaWorkerRequest::new(
        PoissonInlaSpec {
            latent_mean,
            tau_shape,
            tau_rate,
            log_tau_min,
            log_tau_max,
            grid_points,
            endpoint_mass_limit,
            observations,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let identity = PoissonInlaInputIdentity {
        path: input_path.display().to_string(),
        region_count: request.observations.len(),
        observations_sha256: sha256_hex(&serde_json::to_vec(&request.observations)?),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_scipy_pytensor_pymc_inla_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: PoissonInlaWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, identity))
}

fn read_observations(
    path: &std::path::Path,
) -> Result<Vec<PoissonExposureObservation>, BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "Poisson INLA input must be a regular file within the 16 MiB limit",
    )?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["region_id", "count", "exposure"])
    {
        return Err(BayesCliError::Input(
            "Poisson INLA headers must be exactly: region_id,count,exposure".into(),
        ));
    }
    let mut observations = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        let row = row?;
        observations.push(PoissonExposureObservation {
            observation_id: row.region_id,
            count: row.count,
            exposure: row.exposure,
        });
        if observations.len() > 16 {
            return Err(BayesCliError::Input(
                "Poisson INLA region count exceeds 16".into(),
            ));
        }
    }
    Ok(observations)
}

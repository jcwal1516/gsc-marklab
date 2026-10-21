use marklab_bayes::{
    json_sha256, sha256_hex, PoissonExposureObservation, PoissonInlaInputIdentity, PoissonInlaSpec,
    PoissonInlaWorkerRequest, PoissonInlaWorkerResult,
};
use serde::Deserialize;

use super::cli_schema::PoissonLognormalInlaArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    region_id: String,
    count: u64,
    exposure: f64,
}

pub(super) fn run(args: PoissonLognormalInlaArgs) -> Result<(), CliError> {
    let PoissonLognormalInlaArgs {
        input,
        latent_mean,
        tau_shape,
        tau_rate,
        log_tau_min,
        log_tau_max,
        grid_points,
        endpoint_mass_limit,
        timeout_seconds,
        out,
        nuts,
    } = args;
    let input_path = input;
    let sampling = nuts.spec();
    let output_path = out;
    let observations = read_observations(&input_path)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_scipy_pytensor_pymc_inla_worker.py")?;
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
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let identity = PoissonInlaInputIdentity {
        path: input_path.display().to_string(),
        region_count: request.observations.len(),
        observations_sha256: json_sha256(&request.observations)?,
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

fn read_observations(path: &std::path::Path) -> Result<Vec<PoissonExposureObservation>, CliError> {
    crate::input_file::validate_regular_file(
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
        return Err(CliError::Input(
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
            return Err(CliError::Input(
                "Poisson INLA region count exceeds 16".into(),
            ));
        }
    }
    Ok(observations)
}

use marklab_bayes::{
    json_sha256, sha256_hex, PoissonExposureObservation, PoissonLaplaceInputIdentity,
    PoissonLaplaceSpec, PoissonLaplaceWorkerRequest, PoissonLaplaceWorkerResult,
};
use serde::Deserialize;

use super::cli_schema::PoissonLogRateLaplaceArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    observation_id: String,
    count: u64,
    exposure: f64,
}

pub(super) fn run(args: PoissonLogRateLaplaceArgs) -> Result<(), CliError> {
    let PoissonLogRateLaplaceArgs {
        input,
        prior_mean,
        prior_sd,
        initial_log_rate,
        max_iterations,
        gradient_tolerance,
        seed,
        timeout_seconds,
        out,
    } = args;
    let input_path = input;
    let output_path = out;
    let observations = read_observations(&input_path)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_scipy_pytensor_laplace_worker.py")?;
    let request = PoissonLaplaceWorkerRequest::new(
        PoissonLaplaceSpec {
            prior_mean,
            prior_sd,
            initial_log_rate,
            max_iterations,
            gradient_tolerance,
            observations,
        },
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
        seed,
    )?;
    let identity = PoissonLaplaceInputIdentity {
        path: input_path.display().to_string(),
        observation_count: request.observations.len(),
        observations_sha256: json_sha256(&request.observations)?,
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

fn read_observations(path: &std::path::Path) -> Result<Vec<PoissonExposureObservation>, CliError> {
    crate::input_file::validate_regular_file(
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
        return Err(CliError::Input(
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
            return Err(CliError::Input(
                "Poisson Laplace observation count exceeds 100000".into(),
            ));
        }
    }
    Ok(observations)
}

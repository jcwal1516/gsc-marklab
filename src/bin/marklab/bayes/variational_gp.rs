use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, GpObservation, GpPredictionCoordinate, VariationalGpInputIdentity,
    VariationalGpSpec, VariationalGpWorkerRequest, VariationalGpWorkerResult,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    observation_id: String,
    x_um: f64,
    value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionRow {
    prediction_id: String,
    x_um: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    observation_path: PathBuf,
    prediction_path: PathBuf,
    mean_prior_mean: f64,
    mean_prior_sd: f64,
    amplitude_prior_sd: f64,
    length_scale_prior_sd_um: f64,
    noise_prior_sd: f64,
    jitter: f64,
    inducing_points: u32,
    starts: u32,
    iterations: u32,
    learning_rate: f64,
    posterior_draws: u32,
    seed: u64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let observations = read_observations(&observation_path)?;
    let predictions = read_predictions(&prediction_path)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_variational_gp_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = VariationalGpWorkerRequest::new(
        VariationalGpSpec {
            mean_prior_mean,
            mean_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            noise_prior_sd,
            jitter,
            inducing_points,
            starts,
            iterations,
            learning_rate,
            posterior_draws,
            seed,
            observations,
            predictions,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let data_sha256 = sha256_hex(&serde_json::to_vec(&serde_json::json!({
        "observations": &request.observations,
        "predictions": &request.predictions,
    }))?);
    let input = VariationalGpInputIdentity {
        observation_path: observation_path.display().to_string(),
        prediction_path: prediction_path.display().to_string(),
        observations: request.observations.len(),
        predictions: request.predictions.len(),
        data_sha256,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_variational_gp_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: VariationalGpWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, input))
}

fn read_observations(path: &std::path::Path) -> Result<Vec<GpObservation>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["observation_id", "x_um", "value"])
    {
        return Err(BayesCliError::Input(
            "variational GP headers must be exactly: observation_id,x_um,value".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        let row = row?;
        rows.push(GpObservation {
            observation_id: row.observation_id,
            x_um: row.x_um,
            value: row.value,
        });
        if rows.len() > 2_000 {
            return Err(BayesCliError::Input(
                "variational GP observation count exceeds 2000".into(),
            ));
        }
    }
    Ok(rows)
}

fn read_predictions(path: &std::path::Path) -> Result<Vec<GpPredictionCoordinate>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["prediction_id", "x_um"]) {
        return Err(BayesCliError::Input(
            "variational GP prediction headers must be exactly: prediction_id,x_um".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<PredictionRow>() {
        let row = row?;
        rows.push(GpPredictionCoordinate {
            prediction_id: row.prediction_id,
            x_um: row.x_um,
        });
        if rows.len() > 2_048 {
            return Err(BayesCliError::Input(
                "variational GP prediction count exceeds 2048".into(),
            ));
        }
    }
    Ok(rows)
}

fn validate_file(path: &std::path::Path) -> Result<(), BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "variational GP input must be a regular file within the 16 MiB limit",
    )
}

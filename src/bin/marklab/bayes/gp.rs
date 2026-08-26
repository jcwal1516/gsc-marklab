use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, ExactGpInputIdentity, ExactGpSpec, ExactGpWorkerRequest, ExactGpWorkerResult,
    GpObservation, GpPredictionCoordinate, NutsSamplingSpec,
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
    input_path: PathBuf,
    prediction_path: PathBuf,
    mean_prior_mean: f64,
    mean_prior_sd: f64,
    amplitude_prior_sd: f64,
    length_scale_prior_sd_um: f64,
    noise_prior_sd: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let observations = read_observations(&input_path)?;
    let predictions = read_predictions(&prediction_path)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_gp_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = ExactGpWorkerRequest::new(
        ExactGpSpec {
            mean_prior_mean,
            mean_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            noise_prior_sd,
            jitter,
            observations,
            predictions,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let data_sha256 = sha256_hex(&serde_json::to_vec(&serde_json::json!({
        "observations": &request.observations,
        "predictions": &request.predictions,
    }))?);
    let input = ExactGpInputIdentity {
        observation_path: input_path.display().to_string(),
        prediction_path: prediction_path.display().to_string(),
        observations: request.observations.len(),
        predictions: request.predictions.len(),
        data_sha256,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_gp_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: ExactGpWorkerResult = serde_json::from_slice(&result_bytes)?;
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
            "GP observation headers must be exactly: observation_id,x_um,value".into(),
        ));
    }
    let mut values = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        let row = row?;
        values.push(GpObservation {
            observation_id: row.observation_id,
            x_um: row.x_um,
            value: row.value,
        });
        if values.len() > 128 {
            return Err(BayesCliError::Input(
                "exact GP observation count exceeds 128".into(),
            ));
        }
    }
    Ok(values)
}

fn read_predictions(path: &std::path::Path) -> Result<Vec<GpPredictionCoordinate>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["prediction_id", "x_um"]) {
        return Err(BayesCliError::Input(
            "GP prediction headers must be exactly: prediction_id,x_um".into(),
        ));
    }
    let mut values = Vec::new();
    for row in reader.deserialize::<PredictionRow>() {
        let row = row?;
        values.push(GpPredictionCoordinate {
            prediction_id: row.prediction_id,
            x_um: row.x_um,
        });
        if values.len() > 2_048 {
            return Err(BayesCliError::Input(
                "exact GP prediction count exceeds 2048".into(),
            ));
        }
    }
    Ok(values)
}

fn validate_file(path: &std::path::Path) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "GP input must be a regular file within the 16 MiB limit".into(),
        ));
    }
    Ok(())
}

use marklab_bayes::{
    json_sha256, sha256_hex, ExactGpInputIdentity, ExactGpSpec, ExactGpWorkerRequest,
    ExactGpWorkerResult, GpObservation, GpPredictionCoordinate,
};
use serde::Deserialize;

use super::cli_schema::GpRegressionArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

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

pub(super) fn run(args: GpRegressionArgs) -> Result<(), CliError> {
    let GpRegressionArgs {
        input,
        predict,
        mean_prior_mean,
        mean_prior_sd,
        amplitude_prior_sd,
        length_scale_prior_sd_um,
        noise_prior_sd,
        jitter,
        timeout_seconds,
        out,
        nuts,
    } = args;
    let input_path = input;
    let prediction_path = predict;
    let sampling = nuts.spec();
    let output_path = out;
    let observations = read_observations(&input_path)?;
    let predictions = read_predictions(&prediction_path)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_gp_worker.py")?;
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
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let data_sha256 = json_sha256(&serde_json::json!({
        "observations": &request.observations,
        "predictions": &request.predictions,
    }))?;
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

fn read_observations(path: &std::path::Path) -> Result<Vec<GpObservation>, CliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["observation_id", "x_um", "value"])
    {
        return Err(CliError::Input(
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
            return Err(CliError::Input(
                "exact GP observation count exceeds 128".into(),
            ));
        }
    }
    Ok(values)
}

fn read_predictions(path: &std::path::Path) -> Result<Vec<GpPredictionCoordinate>, CliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["prediction_id", "x_um"]) {
        return Err(CliError::Input(
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
            return Err(CliError::Input(
                "exact GP prediction count exceeds 2048".into(),
            ));
        }
    }
    Ok(values)
}

fn validate_file(path: &std::path::Path) -> Result<(), CliError> {
    crate::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "GP input must be a regular file within the 16 MiB limit",
    )
}

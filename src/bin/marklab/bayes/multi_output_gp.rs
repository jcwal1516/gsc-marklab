use marklab_bayes::{
    json_sha256, sha256_hex, GpPredictionCoordinate, MultiOutputGpInputIdentity,
    MultiOutputGpObservation, MultiOutputGpSpec, MultiOutputGpWorkerRequest,
    MultiOutputGpWorkerResult,
};
use serde::Deserialize;

use super::cli_schema::MultiOutputGpArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    coordinate_id: String,
    x_um: f64,
    output_a: f64,
    output_b: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionRow {
    prediction_id: String,
    x_um: f64,
}

pub(super) fn run(args: MultiOutputGpArgs) -> Result<(), CliError> {
    let MultiOutputGpArgs {
        input,
        predict,
        output_a_name,
        output_b_name,
        mean_prior_sd,
        amplitude_prior_sd,
        length_scale_prior_sd_um,
        loading_b_prior_sd,
        noise_a_sd,
        noise_b_sd,
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
    let identity = worker_identity(repository, "marklab_pymc_multi_output_gp_worker.py")?;
    let request = MultiOutputGpWorkerRequest::new(
        MultiOutputGpSpec {
            output_a_name,
            output_b_name,
            mean_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            loading_b_prior_sd,
            noise_a_sd,
            noise_b_sd,
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
    let input = MultiOutputGpInputIdentity {
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
        "marklab_pymc_multi_output_gp_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: MultiOutputGpWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, input))
}

fn read_observations(path: &std::path::Path) -> Result<Vec<MultiOutputGpObservation>, CliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["coordinate_id", "x_um", "output_a", "output_b"])
    {
        return Err(CliError::Input(
            "multi-output GP headers must be exactly: coordinate_id,x_um,output_a,output_b".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        let row = row?;
        rows.push(MultiOutputGpObservation {
            coordinate_id: row.coordinate_id,
            x_um: row.x_um,
            output_a: row.output_a,
            output_b: row.output_b,
        });
        if rows.len() > 64 {
            return Err(CliError::Input(
                "multi-output GP observation count exceeds 64".into(),
            ));
        }
    }
    Ok(rows)
}

fn read_predictions(path: &std::path::Path) -> Result<Vec<GpPredictionCoordinate>, CliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["prediction_id", "x_um"]) {
        return Err(CliError::Input(
            "multi-output GP prediction headers must be exactly: prediction_id,x_um".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<PredictionRow>() {
        let row = row?;
        rows.push(GpPredictionCoordinate {
            prediction_id: row.prediction_id,
            x_um: row.x_um,
        });
        if rows.len() > 1_024 {
            return Err(CliError::Input(
                "multi-output GP prediction count exceeds 1024".into(),
            ));
        }
    }
    Ok(rows)
}

fn validate_file(path: &std::path::Path) -> Result<(), CliError> {
    crate::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "multi-output GP input must be a regular file within the 16 MiB limit",
    )
}

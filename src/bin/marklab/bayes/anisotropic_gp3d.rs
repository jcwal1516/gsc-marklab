use marklab_bayes::{
    json_sha256, sha256_hex, AnisotropicGp3dInputIdentity, AnisotropicGp3dObservation,
    AnisotropicGp3dPredictionCoordinate, AnisotropicGp3dSpec, AnisotropicGp3dWorkerRequest,
    AnisotropicGp3dWorkerResult,
};
use serde::Deserialize;

use super::cli_schema::AnisotropicGp3dArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    observation_id: String,
    x_um: f64,
    y_um: f64,
    z_um: f64,
    value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionRow {
    prediction_id: String,
    x_um: f64,
    y_um: f64,
    z_um: f64,
}

pub(super) fn run(args: AnisotropicGp3dArgs) -> Result<(), CliError> {
    let AnisotropicGp3dArgs {
        input,
        predict,
        mean_prior_mean,
        mean_prior_sd,
        amplitude_prior_sd,
        length_scale_x_prior_sd_um,
        length_scale_y_prior_sd_um,
        length_scale_z_prior_sd_um,
        noise_prior_sd,
        jitter,
        timeout_seconds,
        out,
        nuts,
    } = args;
    let input_path = input;
    let prediction_path = predict;
    let length_scale_prior_sd_um = [
        length_scale_x_prior_sd_um,
        length_scale_y_prior_sd_um,
        length_scale_z_prior_sd_um,
    ];
    let sampling = nuts.spec();
    let output_path = out;
    let observations = read_observations(&input_path)?;
    let predictions = read_predictions(&prediction_path)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_anisotropic_gp3d_worker.py")?;
    let request = AnisotropicGp3dWorkerRequest::new(
        AnisotropicGp3dSpec {
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
    let input = AnisotropicGp3dInputIdentity {
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
        "marklab_pymc_anisotropic_gp3d_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: AnisotropicGp3dWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, input))
}

fn read_observations(path: &std::path::Path) -> Result<Vec<AnisotropicGp3dObservation>, CliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["observation_id", "x_um", "y_um", "z_um", "value"])
    {
        return Err(CliError::Input(
            "anisotropic 3-D GP observation headers must be exactly: observation_id,x_um,y_um,z_um,value"
                .into(),
        ));
    }
    let mut values = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        let row = row?;
        values.push(AnisotropicGp3dObservation {
            observation_id: row.observation_id,
            coordinates_um: [row.x_um, row.y_um, row.z_um],
            value: row.value,
        });
        if values.len() > 64 {
            return Err(CliError::Input(
                "anisotropic 3-D GP observation count exceeds 64".into(),
            ));
        }
    }
    Ok(values)
}

fn read_predictions(
    path: &std::path::Path,
) -> Result<Vec<AnisotropicGp3dPredictionCoordinate>, CliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["prediction_id", "x_um", "y_um", "z_um"])
    {
        return Err(CliError::Input(
            "anisotropic 3-D GP prediction headers must be exactly: prediction_id,x_um,y_um,z_um"
                .into(),
        ));
    }
    let mut values = Vec::new();
    for row in reader.deserialize::<PredictionRow>() {
        let row = row?;
        values.push(AnisotropicGp3dPredictionCoordinate {
            prediction_id: row.prediction_id,
            coordinates_um: [row.x_um, row.y_um, row.z_um],
        });
        if values.len() > 512 {
            return Err(CliError::Input(
                "anisotropic 3-D GP prediction count exceeds 512".into(),
            ));
        }
    }
    Ok(values)
}

fn validate_file(path: &std::path::Path) -> Result<(), CliError> {
    crate::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "anisotropic 3-D GP input must be a regular file within the 16 MiB limit",
    )
}

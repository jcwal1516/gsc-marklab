use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, AnisotropicGp3dInputIdentity, AnisotropicGp3dObservation,
    AnisotropicGp3dPredictionCoordinate, AnisotropicGp3dSpec, AnisotropicGp3dWorkerRequest,
    AnisotropicGp3dWorkerResult, NutsSamplingSpec,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

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

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    prediction_path: PathBuf,
    mean_prior_mean: f64,
    mean_prior_sd: f64,
    amplitude_prior_sd: f64,
    length_scale_prior_sd_um: [f64; 3],
    noise_prior_sd: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let observations = read_observations(&input_path)?;
    let predictions = read_predictions(&prediction_path)?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_anisotropic_gp3d_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
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
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let data_sha256 = sha256_hex(&serde_json::to_vec(&serde_json::json!({
        "observations": &request.observations,
        "predictions": &request.predictions,
    }))?);
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

fn read_observations(
    path: &std::path::Path,
) -> Result<Vec<AnisotropicGp3dObservation>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["observation_id", "x_um", "y_um", "z_um", "value"])
    {
        return Err(BayesCliError::Input(
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
            return Err(BayesCliError::Input(
                "anisotropic 3-D GP observation count exceeds 64".into(),
            ));
        }
    }
    Ok(values)
}

fn read_predictions(
    path: &std::path::Path,
) -> Result<Vec<AnisotropicGp3dPredictionCoordinate>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["prediction_id", "x_um", "y_um", "z_um"])
    {
        return Err(BayesCliError::Input(
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
            return Err(BayesCliError::Input(
                "anisotropic 3-D GP prediction count exceeds 512".into(),
            ));
        }
    }
    Ok(values)
}

fn validate_file(path: &std::path::Path) -> Result<(), BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "anisotropic 3-D GP input must be a regular file within the 16 MiB limit",
    )
}

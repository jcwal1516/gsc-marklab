use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, PredictionCalibrationMetrics, PredictionCalibrationRow, PredictionCalibrationSpec,
    PredictionCalibrationWorkerRequest, PredictionCalibrationWorkerResult,
};
use serde::{Deserialize, Serialize};

use super::{embedding_spatial, publish_json, run_worker, BayesCliError};

#[derive(Debug, Deserialize)]
struct InputRow {
    patient_id: String,
    split: String,
    score: f64,
    label: u8,
}

pub(super) fn run(
    input: PathBuf,
    method: String,
    bins: u32,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    if method != "platt-logistic" {
        return Err(BayesCliError::Input(
            "calibration method must be platt-logistic".into(),
        ));
    }
    let input_bytes = embedding_spatial::read(&input)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    if !reader
        .headers()?
        .iter()
        .eq(["patient_id", "split", "score", "label"])
    {
        return Err(BayesCliError::Input(
            "calibration CSV header differs".into(),
        ));
    }
    let rows = reader
        .deserialize::<InputRow>()
        .map(|row| {
            let row = row?;
            Ok(PredictionCalibrationRow {
                patient_id: row.patient_id,
                split: row.split,
                score: row.score,
                label: row.label,
            })
        })
        .collect::<Result<Vec<_>, csv::Error>>()?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_name = "marklab_scipy_prediction_calibration_worker.py";
    let worker_path = worker_directory.join(worker_name);
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = PredictionCalibrationWorkerRequest::new(
        PredictionCalibrationSpec {
            rows,
            bins,
            timeout_seconds,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, worker_name, &request_bytes, timeout_seconds)?;
    let result: PredictionCalibrationWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.prediction_calibration",
            version: 1,
            backend: result.backend,
            input_sha256: sha256_hex(&input_bytes),
            request_sha256: result.request_sha256,
            method: "platt_logistic",
            fit_split: "training_oof",
            evaluation_split: "test",
            calibrator: result.calibrator,
            predictions: result.predictions,
            metrics: result.metrics,
        },
    )
}

#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    backend: marklab_bayes::WorkerBackend,
    input_sha256: String,
    request_sha256: String,
    method: &'static str,
    fit_split: &'static str,
    evaluation_split: &'static str,
    calibrator: marklab_bayes::PlattCalibrator,
    predictions: Vec<marklab_bayes::CalibratedPrediction>,
    metrics: PredictionCalibrationMetrics,
}

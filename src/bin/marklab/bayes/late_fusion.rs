use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, LateFusionPatient, LateFusionSpec, LateFusionWorkerRequest, LateFusionWorkerResult,
};
use serde::Serialize;

use super::{embedding_spatial, publish_json, run_worker, BayesCliError};

pub(super) fn run(
    input: PathBuf,
    l2_penalty: f64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let input_bytes = embedding_spatial::read(&input)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let headers = reader.headers()?.clone();
    if headers.len() < 6
        || headers.iter().take(4).collect::<Vec<_>>()
            != ["patient_id", "split", "base_prediction_source", "label"]
    {
        return Err(BayesCliError::Input(
            "late-fusion CSV header differs".into(),
        ));
    }
    let modalities = headers
        .iter()
        .skip(4)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut patients = Vec::new();
    for row in reader.records() {
        let row = row?;
        patients.push(LateFusionPatient {
            patient_id: row[0].into(),
            split: row[1].into(),
            base_prediction_source: row[2].into(),
            label: row[3]
                .parse()
                .map_err(|_| BayesCliError::Input("late-fusion label is invalid".into()))?,
            modality_probabilities: row
                .iter()
                .skip(4)
                .map(|value| {
                    if value.is_empty() {
                        Ok(None)
                    } else {
                        value.parse().map(Some).map_err(|_| {
                            BayesCliError::Input("late-fusion probability is invalid".into())
                        })
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_name = "marklab_scipy_late_fusion_worker.py";
    let worker_path = worker_directory.join(worker_name);
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = LateFusionWorkerRequest::new(
        LateFusionSpec {
            patients,
            modalities,
            l2_penalty,
            timeout_seconds,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, worker_name, &request_bytes, timeout_seconds)?;
    let result: LateFusionWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.late_fusion",
            version: 1,
            backend: result.backend,
            input_sha256: sha256_hex(&input_bytes),
            request_sha256: result.request_sha256,
            base_prediction_source: request.base_prediction_source,
            meta_fit_split: "meta_train",
            calibration_split: "calibration",
            evaluation_split: "test",
            modalities: request.modalities,
            model: result.model,
            calibrator: result.calibrator,
            predictions: result.predictions,
            metrics: result.metrics,
            missing_scenarios: result.missing_scenarios,
            modality_ablations: result.modality_ablations,
            claim_status: "experimental_patient_level_late_fusion",
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
    base_prediction_source: &'static str,
    meta_fit_split: &'static str,
    calibration_split: &'static str,
    evaluation_split: &'static str,
    modalities: Vec<String>,
    model: marklab_bayes::LateFusionModel,
    calibrator: marklab_bayes::LateFusionCalibrator,
    predictions: Vec<marklab_bayes::LateFusionPrediction>,
    metrics: marklab_bayes::LateFusionMetric,
    missing_scenarios: Vec<marklab_bayes::MissingScenario>,
    modality_ablations: Vec<marklab_bayes::ModalityAblation>,
    claim_status: &'static str,
}

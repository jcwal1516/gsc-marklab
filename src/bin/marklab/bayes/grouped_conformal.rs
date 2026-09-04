use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, GroupedConformalPatient, GroupedConformalSpec, GroupedConformalWorkerRequest,
    GroupedConformalWorkerResult,
};
use serde::Serialize;

use super::{embedding_spatial, publish_json, run_worker, BayesCliError};

pub(super) fn run(
    input: PathBuf,
    alpha: f64,
    l2_penalty: f64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let input_bytes = embedding_spatial::read(&input)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let headers = reader.headers()?.clone();
    if headers.len() < 7
        || headers.iter().take(5).collect::<Vec<_>>()
            != ["patient_id", "split", "site", "subgroup", "label"]
    {
        return Err(BayesCliError::Input(
            "grouped conformal CSV header differs".into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(5)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut patients = Vec::new();
    for row in reader.records() {
        let row = row?;
        patients.push(GroupedConformalPatient {
            patient_id: row[0].into(),
            split: row[1].into(),
            site: row[2].into(),
            subgroup: row[3].into(),
            label: row[4]
                .parse()
                .map_err(|_| BayesCliError::Input("conformal label is invalid".into()))?,
            features: row
                .iter()
                .skip(5)
                .map(|value| {
                    value
                        .parse()
                        .map_err(|_| BayesCliError::Input("conformal feature is invalid".into()))
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_name = "marklab_scipy_grouped_conformal_worker.py";
    let worker_path = worker_directory.join(worker_name);
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = GroupedConformalWorkerRequest::new(
        GroupedConformalSpec {
            patients,
            feature_names,
            alpha,
            l2_penalty,
            timeout_seconds,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, worker_name, &request_bytes, timeout_seconds)?;
    let result: GroupedConformalWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.grouped_conformal_prediction",
            version: 1,
            backend: result.backend,
            input_sha256: sha256_hex(&input_bytes),
            request_sha256: result.request_sha256,
            feature_names: request.feature_names,
            alpha: request.alpha,
            fit_split: "train",
            quantile_split: "calibration",
            prediction_split: "test",
            model: result.model,
            calibration_count: result.calibration_count,
            corrected_rank: result.corrected_rank,
            nonconformity_threshold: result.nonconformity_threshold,
            predictions: result.predictions,
            coverage: result.coverage,
            claim_status: "exchangeability_conditional_coverage_not_guaranteed_under_shift",
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
    feature_names: Vec<String>,
    alpha: f64,
    fit_split: &'static str,
    quantile_split: &'static str,
    prediction_split: &'static str,
    model: marklab_bayes::GroupedConformalModel,
    calibration_count: u32,
    corrected_rank: u32,
    nonconformity_threshold: f64,
    predictions: Vec<marklab_bayes::GroupedConformalPrediction>,
    coverage: marklab_bayes::GroupedCoverage,
    claim_status: &'static str,
}

use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, PredictiveStackingPatient, PredictiveStackingSpec, PredictiveStackingWorkerRequest,
    PredictiveStackingWorkerResult,
};
use serde::Serialize;

use super::{embedding_spatial, publish_json, run_worker, BayesCliError};

pub(super) fn run(input: PathBuf, timeout_seconds: u64, out: PathBuf) -> Result<(), BayesCliError> {
    let input_bytes = embedding_spatial::read(&input)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let headers = reader.headers()?.clone();
    if headers.len() < 4
        || headers.iter().take(2).collect::<Vec<_>>() != ["patient_id", "held_out_unit"]
    {
        return Err(BayesCliError::Input(
            "predictive-stacking CSV header differs".into(),
        ));
    }
    let model_names = headers
        .iter()
        .skip(2)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut patients = Vec::new();
    for row in reader.records() {
        let row = row?;
        patients.push(PredictiveStackingPatient {
            patient_id: row[0].into(),
            held_out_unit: row[1].into(),
            log_predictive_densities: row
                .iter()
                .skip(2)
                .map(|value| {
                    value.parse().map_err(|_| {
                        BayesCliError::Input("predictive-stacking log density is invalid".into())
                    })
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
    let worker_name = "marklab_scipy_predictive_stacking_worker.py";
    let worker_path = worker_directory.join(worker_name);
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = PredictiveStackingWorkerRequest::new(
        PredictiveStackingSpec {
            patients,
            model_names,
            timeout_seconds,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, worker_name, &request_bytes, timeout_seconds)?;
    let result: PredictiveStackingWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.predictive_stacking",
            version: 1,
            backend: result.backend,
            input_sha256: sha256_hex(&input_bytes),
            request_sha256: result.request_sha256,
            held_out_unit: "patient",
            patient_count: request.patients.len() as u32,
            weights: result.weights,
            objective_sum_log_predictive_density: result.objective_sum_log_predictive_density,
            grouped_mixture_log_predictive_density: result.grouped_mixture_log_predictive_density,
            leave_one_patient_out_sensitivity: result.leave_one_patient_out_sensitivity,
            weight_interpretation:
                "predictive_optimization_weights_not_posterior_model_probabilities",
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
    held_out_unit: &'static str,
    patient_count: u32,
    weights: Vec<marklab_bayes::StackingWeight>,
    objective_sum_log_predictive_density: f64,
    grouped_mixture_log_predictive_density: Vec<marklab_bayes::StackingPatientDensity>,
    leave_one_patient_out_sensitivity: Vec<marklab_bayes::StackingWeightSensitivity>,
    weight_interpretation: &'static str,
}

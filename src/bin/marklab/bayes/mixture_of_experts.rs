use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, MixtureOfExpertsPatient, MixtureOfExpertsSpec, MixtureOfExpertsWorkerRequest,
    MixtureOfExpertsWorkerResult,
};
use serde::Serialize;

use super::{embedding_spatial, publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input: PathBuf,
    l2_penalty: f64,
    entropy_regularization: f64,
    ood_validation_quantile: f64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let input_bytes = embedding_spatial::read(&input)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let headers = reader.headers()?.clone();
    if headers.len() < 7
        || headers.iter().take(4).collect::<Vec<_>>()
            != ["patient_id", "split", "expert_prediction_source", "label"]
    {
        return Err(BayesCliError::Input(
            "mixture-of-experts CSV header differs".into(),
        ));
    }
    let context_end = headers
        .iter()
        .enumerate()
        .skip(4)
        .take_while(|(_, name)| name.starts_with("context_"))
        .last()
        .map_or(4, |(index, _)| index + 1);
    let context_names = headers
        .iter()
        .skip(4)
        .take(context_end - 4)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let expert_names = headers
        .iter()
        .skip(context_end)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut patients = Vec::new();
    for row in reader.records() {
        let row = row?;
        patients.push(MixtureOfExpertsPatient {
            patient_id: row[0].into(),
            split: row[1].into(),
            expert_prediction_source: row[2].into(),
            label: row[3]
                .parse()
                .map_err(|_| BayesCliError::Input("mixture label is invalid".into()))?,
            context: row
                .iter()
                .skip(4)
                .take(context_end - 4)
                .map(|value| {
                    value
                        .parse()
                        .map_err(|_| BayesCliError::Input("mixture context is invalid".into()))
                })
                .collect::<Result<Vec<_>, _>>()?,
            expert_probabilities: row
                .iter()
                .skip(context_end)
                .map(|value| {
                    if value.is_empty() {
                        Ok(None)
                    } else {
                        value.parse().map(Some).map_err(|_| {
                            BayesCliError::Input("mixture expert probability is invalid".into())
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
    let worker_name = "marklab_scipy_mixture_of_experts_worker.py";
    let worker_path = worker_directory.join(worker_name);
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = MixtureOfExpertsWorkerRequest::new(
        MixtureOfExpertsSpec {
            patients,
            context_names,
            expert_names,
            l2_penalty,
            entropy_regularization,
            ood_validation_quantile,
            timeout_seconds,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, worker_name, &request_bytes, timeout_seconds)?;
    let result: MixtureOfExpertsWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.mixture_of_experts_fusion",
            version: 1,
            backend: result.backend,
            input_sha256: sha256_hex(&input_bytes),
            request_sha256: result.request_sha256,
            expert_prediction_source: "patient_level_out_of_fold",
            gate_fit_split: "gate_train",
            calibration_split: "calibration",
            evaluation_split: "test",
            experts: request.expert_names,
            gating_features: result.model.gating_feature_names.clone(),
            model: result.model,
            calibrator: result.calibrator,
            ood_threshold: result.ood_threshold,
            predictions: result.predictions,
            metrics: result.metrics,
            claim_status: "experimental_context_gated_predictive_mixture",
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
    expert_prediction_source: &'static str,
    gate_fit_split: &'static str,
    calibration_split: &'static str,
    evaluation_split: &'static str,
    experts: Vec<String>,
    gating_features: Vec<String>,
    model: marklab_bayes::MixtureGateModel,
    calibrator: marklab_bayes::MixtureCalibrator,
    ood_threshold: f64,
    predictions: Vec<marklab_bayes::MixturePrediction>,
    metrics: marklab_bayes::MixtureMetrics,
    claim_status: &'static str,
}

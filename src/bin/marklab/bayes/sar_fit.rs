use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NutsSamplingSpec, SarFitInputIdentity, SarFitSpec, SarFitWorkerRequest,
    SarFitWorkerResult, SarModelType,
};

use super::{publish_json, run_worker, sar, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    region_path: PathBuf,
    edge_path: PathBuf,
    data_path: PathBuf,
    model: sar::CliSarModel,
    interpretation: sar::CliSarInterpretation,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_sd: f64,
    rho_bound: f64,
    sigma_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let _interpretation = match interpretation {
        sar::CliSarInterpretation::Descriptive => "descriptive",
    };
    let input = sar::read_input(&region_path, &edge_path, &data_path)?;
    let dimension = input.weights.region_ids.len();
    let mut dense_weights = vec![0.0; dimension * dimension];
    for edge in &input.weights.weights {
        dense_weights[edge.source_index * dimension + edge.target_index] = edge.weight;
    }
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_sar_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let data_sha256 = sha256_hex(&serde_json::to_vec(&serde_json::json!({
        "region_ids": input.weights.region_ids,
        "response": input.response,
        "design": input.design,
        "predictor_names": input.predictor_names,
    }))?);
    let request = SarFitWorkerRequest::new(
        SarFitSpec {
            model_type: match model {
                sar::CliSarModel::Lag => SarModelType::Lag,
                sar::CliSarModel::Error => SarModelType::Error,
            },
            region_ids: input.weights.region_ids,
            weights: dense_weights,
            weights_digest_sha256: input.weights.digest_sha256.clone(),
            response: input.response,
            design: input.design,
            predictor_names: input.predictor_names,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            rho_bound,
            sigma_prior_sd,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let identity = SarFitInputIdentity {
        regions_path: region_path.display().to_string(),
        edges_path: edge_path.display().to_string(),
        data_path: data_path.display().to_string(),
        weights_digest_sha256: request.weights_digest_sha256.clone(),
        data_sha256,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_sar_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: SarFitWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, identity))
}

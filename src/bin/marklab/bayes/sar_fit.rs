use marklab_bayes::{
    json_sha256, sha256_hex, SarFitInputIdentity, SarFitSpec, SarFitWorkerRequest,
    SarFitWorkerResult, SarModelType,
};

use super::cli_schema::SarFitArgs;
use super::worker_identity;
use super::{publish_json, run_worker, sar, CliError};

pub(super) fn run(args: SarFitArgs) -> Result<(), CliError> {
    let SarFitArgs {
        regions,
        edges,
        data,
        model,
        interpretation,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_sd,
        rho_bound,
        sigma_prior_sd,
        timeout_seconds,
        out,
        nuts,
    } = args;
    let region_path = regions;
    let edge_path = edges;
    let data_path = data;
    let sampling = nuts.spec();
    let output_path = out;
    let _interpretation = match interpretation {
        sar::CliSarInterpretation::Descriptive => "descriptive",
    };
    let input = sar::read_input(&region_path, &edge_path, &data_path)?;
    let dimension = input.weights.region_ids.len();
    let mut dense_weights = vec![0.0; dimension * dimension];
    for edge in &input.weights.weights {
        dense_weights[edge.source_index * dimension + edge.target_index] = edge.weight;
    }
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_sar_worker.py")?;
    let data_sha256 = json_sha256(&serde_json::json!({
        "region_ids": input.weights.region_ids,
        "response": input.response,
        "design": input.design,
        "predictor_names": input.predictor_names,
    }))?;
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
        identity.environment_lock_sha256,
        identity.worker_sha256,
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

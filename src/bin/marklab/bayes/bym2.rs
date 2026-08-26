use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, Bym2FitSpec, Bym2FitWorkerRequest, Bym2FitWorkerResult, Bym2InputIdentity,
    NutsSamplingSpec,
};

use super::{bym, publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    region_path: PathBuf,
    edge_path: PathBuf,
    data_path: PathBuf,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_sd: f64,
    total_sd_prior: f64,
    phi_alpha: f64,
    phi_beta: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input = bym::read_input(&region_path, &edge_path, &data_path)?;
    let weights = input.weights;
    let plan = input.plan;
    let data = input.data;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_bym2_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let data_sha256 = sha256_hex(&serde_json::to_vec(&serde_json::json!({
        "region_ids": weights.region_ids,
        "counts": data.counts,
        "expected": data.expected,
        "design": data.design,
        "predictor_names": data.predictor_names,
    }))?);
    let request = Bym2FitWorkerRequest::new(
        Bym2FitSpec {
            region_ids: weights.region_ids,
            weights_digest_sha256: weights.digest_sha256.clone(),
            scaled_icar_transform: plan.scaled_transform,
            rank_deficiency: plan.rank_deficiency,
            components: plan.components,
            original_typical_marginal_variance: plan.typical_marginal_variance,
            scaled_typical_marginal_variance: plan.scaled_typical_marginal_variance,
            counts: data.counts,
            expected: data.expected,
            design: data.design,
            predictor_names: data.predictor_names,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            total_sd_prior,
            phi_alpha,
            phi_beta,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let identity = Bym2InputIdentity {
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
        "marklab_pymc_bym2_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: Bym2FitWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, identity))
}

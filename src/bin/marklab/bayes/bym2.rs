use marklab_bayes::{
    json_sha256, sha256_hex, Bym2FitSpec, Bym2FitWorkerRequest, Bym2FitWorkerResult,
    Bym2InputIdentity,
};

use super::cli_schema::Bym2FitArgs;
use super::worker_identity;
use super::{bym, publish_json, run_worker, CliError};

pub(super) fn run(args: Bym2FitArgs) -> Result<(), CliError> {
    let Bym2FitArgs {
        regions,
        edges,
        data,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_sd,
        total_sd_prior,
        phi_alpha,
        phi_beta,
        timeout_seconds,
        out,
        nuts,
    } = args;
    let region_path = regions;
    let edge_path = edges;
    let data_path = data;
    let sampling = nuts.spec();
    let output_path = out;
    let input = bym::read_input(&region_path, &edge_path, &data_path)?;
    let weights = input.weights;
    let plan = input.plan;
    let data = input.data;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_bym2_worker.py")?;
    let data_sha256 = json_sha256(&serde_json::json!({
        "region_ids": weights.region_ids,
        "counts": data.counts,
        "expected": data.expected,
        "design": data.design,
        "predictor_names": data.predictor_names,
    }))?;
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
        identity.environment_lock_sha256,
        identity.worker_sha256,
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

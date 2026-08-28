use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NumpyroDirichletMultinomialGroupSbcWorkerRequest,
    NumpyroDirichletMultinomialGroupSbcWorkerResult, NutsSamplingSpec,
};

use super::{dirichlet_multinomial_group, publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    logit_prior_sd: f64,
    group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    replicates: u32,
    sampling: NutsSamplingSpec,
    minimum_rank_uniformity_p_value: f64,
    minimum_coverage_90: f64,
    maximum_coverage_90: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = dirichlet_multinomial_group::prepare(
        input_path,
        reference_group,
        comparison_group,
        logit_prior_sd,
        group_effect_prior_sd,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path =
        worker_directory.join("marklab_numpyro_dirichlet_multinomial_group_sbc_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NumpyroDirichletMultinomialGroupSbcWorkerRequest::new(
        prepared.request,
        replicates,
        minimum_rank_uniformity_p_value,
        minimum_coverage_90,
        maximum_coverage_90,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_numpyro_dirichlet_multinomial_group_sbc_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NumpyroDirichletMultinomialGroupSbcWorkerResult =
        serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request))
}

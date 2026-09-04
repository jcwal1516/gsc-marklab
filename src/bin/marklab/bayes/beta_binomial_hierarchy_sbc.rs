use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NumpyroBetaBinomialSbcWorkerRequest, NumpyroBetaBinomialSbcWorkerResult,
    NutsSamplingSpec,
};

use super::{beta_binomial_hierarchy, publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    population_alpha: f64,
    population_beta: f64,
    concentration_prior_sd: f64,
    replicates: u32,
    sampling: NutsSamplingSpec,
    minimum_rank_uniformity_p_value: f64,
    minimum_coverage_90: f64,
    maximum_coverage_90: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = beta_binomial_hierarchy::prepare(
        input_path,
        population_alpha,
        population_beta,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path =
        worker_directory.join("marklab_numpyro_beta_binomial_hierarchy_sbc_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NumpyroBetaBinomialSbcWorkerRequest::new(
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
        "marklab_numpyro_beta_binomial_hierarchy_sbc_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NumpyroBetaBinomialSbcWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request))
}

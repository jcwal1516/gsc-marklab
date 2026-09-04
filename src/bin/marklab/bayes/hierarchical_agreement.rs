use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, HierarchicalAgreementPolicy, HierarchicalAgreementResult,
    NumpyroHierarchyWorkerRequest, NumpyroHierarchyWorkerResult, NutsSamplingSpec,
};

use super::{hierarchical, publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    maximum_standardized_difference: f64,
    minimum_absolute_tolerance: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = hierarchical::prepare(
        input_path,
        global_prior_mean,
        global_prior_sd,
        between_patient_sd_prior,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    let pymc = hierarchical::execute(&prepared)?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_numpyro_hierarchical_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let numpyro_request = NumpyroHierarchyWorkerRequest::new(
        prepared.request.clone(),
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&numpyro_request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_numpyro_hierarchical_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let numpyro: NumpyroHierarchyWorkerResult = serde_json::from_slice(&result_bytes)?;
    numpyro.validate(&numpyro_request, &request_sha256)?;
    let result = HierarchicalAgreementResult::new(
        prepared.request,
        prepared.input_identity,
        pymc,
        numpyro,
        HierarchicalAgreementPolicy {
            maximum_standardized_difference,
            minimum_absolute_tolerance,
        },
    )?;
    publish_json(&output_path, &result)
}

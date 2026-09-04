use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NumpyroHierarchySbcWorkerRequest, NumpyroHierarchySbcWorkerResult, NutsSamplingSpec,
};

use super::{publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    known_sigma: f64,
    patients: u32,
    observations_per_patient: u32,
    replicates: u32,
    sampling: NutsSamplingSpec,
    minimum_rank_uniformity_p_value: f64,
    minimum_coverage_90: f64,
    maximum_coverage_90: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_numpyro_hierarchical_sbc_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NumpyroHierarchySbcWorkerRequest::new(
        global_prior_mean,
        global_prior_sd,
        between_patient_sd_prior,
        known_sigma,
        patients,
        observations_per_patient,
        replicates,
        sampling,
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
        "marklab_numpyro_hierarchical_sbc_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NumpyroHierarchySbcWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request))
}

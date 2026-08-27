use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, GriddedLgcpAgreementPolicy, GriddedLgcpAgreementResult,
    NumpyroGriddedLgcpWorkerRequest, NumpyroGriddedLgcpWorkerResult, NutsSamplingSpec,
};

use super::{gridded_lgcp_fit, publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    events_path: PathBuf,
    grid_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    grid_x: u32,
    grid_y: u32,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    maximum_standardized_difference: f64,
    minimum_parameter_tolerance: f64,
    minimum_field_tolerance: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = gridded_lgcp_fit::prepare(
        events_path,
        grid_path,
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        grid_x,
        grid_y,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        sampling,
        timeout_seconds,
        None,
    )?;
    let pymc = gridded_lgcp_fit::execute(&prepared)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_numpyro_gridded_lgcp_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let numpyro_request = NumpyroGriddedLgcpWorkerRequest::new(
        prepared.request.clone(),
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&numpyro_request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_numpyro_gridded_lgcp_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let numpyro: NumpyroGriddedLgcpWorkerResult = serde_json::from_slice(&result_bytes)?;
    let numpyro = numpyro.into_validated_payload(&numpyro_request, &request_sha256)?;
    let result = GriddedLgcpAgreementResult::new(
        prepared.request,
        prepared.input_identity,
        pymc,
        numpyro,
        GriddedLgcpAgreementPolicy {
            maximum_standardized_difference,
            minimum_parameter_tolerance,
            minimum_field_tolerance,
        },
    )?;
    publish_json(&output_path, &result)
}

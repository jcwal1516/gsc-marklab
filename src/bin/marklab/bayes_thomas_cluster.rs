//! `bayes advanced-cluster` and `bayes advanced-cluster-sbc`: the Bayesian Thomas cluster
//! process worker behind a validated, budgeted request.

use std::path::Path;

use marklab_bayes::advanced::thomas_cluster::{
    ThomasClusterInput, ThomasClusterRequest, ThomasClusterResult, ThomasClusterSbcInput,
    ThomasClusterSbcResult, THOMAS_WORKER,
};
use marklab_core::backend::{sha256_hex, WorkerError};

use super::topology::{publish_json, read_input, read_required, run_worker};
use crate::cli_error::CliError;

struct WorkerAssets {
    repository: std::path::PathBuf,
    worker: std::path::PathBuf,
    lock_sha256: String,
    worker_sha256: String,
}

fn worker_assets() -> Result<WorkerAssets, CliError> {
    let repository = marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let lock = read_required(&directory.join("uv.lock"))?;
    let worker = directory.join(THOMAS_WORKER);
    let worker_bytes = read_required(&worker)?;
    Ok(WorkerAssets {
        repository,
        worker,
        lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker_bytes),
    })
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8], context: &str) -> Result<T, CliError> {
    serde_json::from_slice(bytes).map_err(|error| {
        CliError::Json(<serde_json::Error as serde::de::Error>::custom(format!(
            "{context} schema mismatch: {error}"
        )))
    })
}

fn worker_error(error: WorkerError) -> CliError {
    match error {
        WorkerError::InvalidSpec(message) => CliError::Input(message),
        other => CliError::Backend(other.to_string()),
    }
}

pub(crate) fn run_fit(input: &Path, out: &Path) -> Result<(), CliError> {
    let input: ThomasClusterInput = decode(&read_input(input)?, "Thomas cluster input")?;
    let timeout = input.timeout_seconds;
    let assets = worker_assets()?;
    let request = ThomasClusterRequest::fit(input, assets.lock_sha256, assets.worker_sha256)
        .map_err(worker_error)?;
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(&assets.repository, &assets.worker, &request_bytes, timeout)?;
    let result: ThomasClusterResult = decode(&response, "Thomas cluster result")?;
    result
        .validate(&request, &sha256_hex(&request_bytes))
        .map_err(worker_error)?;
    publish_json(out, &result)
}

pub(crate) fn run_sbc(input: &Path, out: &Path) -> Result<(), CliError> {
    let input: ThomasClusterSbcInput = decode(&read_input(input)?, "Thomas cluster SBC input")?;
    let timeout = input.timeout_seconds;
    let assets = worker_assets()?;
    let request = ThomasClusterRequest::sbc(input, assets.lock_sha256, assets.worker_sha256)
        .map_err(worker_error)?;
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(&assets.repository, &assets.worker, &request_bytes, timeout)?;
    let result: ThomasClusterSbcResult = decode(&response, "Thomas cluster SBC result")?;
    result
        .validate(&request, &sha256_hex(&request_bytes))
        .map_err(worker_error)?;
    publish_json(out, &result)
}

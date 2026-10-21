use std::path::{Path, PathBuf};

use marklab_topology::{
    sha256_hex, WitnessPersistenceResult, WitnessPersistenceSpec, WitnessPersistenceWorkerRequest,
};

use super::{publish_json, read_input, read_required, run_worker, CliError};

pub(crate) struct PreparedWitnessPersistence {
    pub(crate) request: WitnessPersistenceWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(super) timeout_seconds: u64,
}

pub(crate) fn prepare_witness_persistence(
    input: &Path,
) -> Result<PreparedWitnessPersistence, CliError> {
    let bytes = read_input(input)?;
    let spec: WitnessPersistenceSpec = serde_json::from_slice(&bytes)?;
    let repository = marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_gudhi_witness_persistence_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let timeout_seconds = spec.timeout_seconds;
    let request =
        WitnessPersistenceWorkerRequest::new(spec, sha256_hex(&lock), sha256_hex(&worker))
            .map_err(|error| CliError::Input(error.to_string()))?;
    let request_bytes = serde_json::to_vec(&request)?;
    Ok(PreparedWitnessPersistence {
        request,
        request_bytes,
        timeout_seconds,
    })
}

pub(crate) fn execute_witness_persistence(
    prepared: &PreparedWitnessPersistence,
) -> Result<WitnessPersistenceResult, CliError> {
    let repository = marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_gudhi_witness_persistence_worker.py");
    if sha256_hex(&read_required(&lock_path)?) != prepared.request.backend.environment_lock_sha256
        || sha256_hex(&read_required(&worker_path)?) != prepared.request.backend.worker_sha256
    {
        return Err(CliError::Backend(
            "GUDHI environment or worker changed after request preparation".into(),
        ));
    }
    let response = run_worker(
        &repository,
        &worker_path,
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    WitnessPersistenceResult::parse_and_validate(
        &response,
        &prepared.request,
        &prepared.request_bytes,
    )
    .map_err(|error| CliError::Backend(error.to_string()))
}

pub(super) fn run_witness_persistence(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let prepared = prepare_witness_persistence(&input)?;
    let result = execute_witness_persistence(&prepared)?;
    publish_json(&out, &result)
}

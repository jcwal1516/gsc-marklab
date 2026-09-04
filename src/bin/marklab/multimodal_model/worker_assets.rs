use std::path::PathBuf;

use marklab_topology::sha256_hex;

use super::super::topology::{read_required, TopologyCliError};

pub(super) struct WorkerAssets {
    pub(super) repository: PathBuf,
    pub(super) worker_path: PathBuf,
    pub(super) lock_sha256: String,
    pub(super) worker_sha256: String,
}

pub(super) fn load(worker_name: &str) -> Result<WorkerAssets, TopologyCliError> {
    let repository = marklab::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python").join(worker_name);
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    Ok(WorkerAssets {
        repository,
        worker_path,
        lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker),
    })
}

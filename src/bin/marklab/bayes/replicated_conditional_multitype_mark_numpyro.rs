use marklab_bayes::{sha256_hex, BackendContract};

use super::{replicated_conditional_multitype_mark::Prepared, run_worker, CliError};
pub(crate) use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;

const MAXIMUM_ADAPTER_BYTES: u64 = 8 * 1_048_576;

pub(crate) struct Execution {
    pub(crate) result: serde_json::Value,
    pub(crate) request_sha256: String,
    pub(crate) backend: BackendContract,
}

pub(crate) fn execute(
    prepared: &Prepared,
    depth: u32,
    timeout_seconds: u64,
) -> Result<Execution, CliError> {
    if !(10..=14).contains(&depth) {
        return Err(CliError::Input(
            "NumPyro replicated conditional-mark depth is invalid".into(),
        ));
    }
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_bounded(
            &directory.join("uv.lock"),
            MAXIMUM_ADAPTER_BYTES,
        )?),
        worker_sha256: sha256_hex(&read_bounded(
            &directory.join("marklab_numpyro_replicated_conditional_multitype_mark_worker.py"),
            MAXIMUM_ADAPTER_BYTES,
        )?),
    };
    let request = serde_json::json!({
        "format": "marklab.numpyro_replicated_conditional_multitype_mark_request",
        "version": 1,
        "backend": backend,
        "jax_version": JAX_VERSION,
        "source_request_sha256": prepared.request_sha256(),
        "source_request": serde_json::from_slice::<serde_json::Value>(prepared.request_bytes())?,
        "maximum_tree_depth": depth,
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let output = run_worker(
        repository,
        "marklab_numpyro_replicated_conditional_multitype_mark_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    Ok(Execution {
        result: serde_json::from_slice(&output)?,
        request_sha256,
        backend,
    })
}

fn read_bounded(path: &std::path::Path, maximum: u64) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        maximum,
        "NumPyro adapter source is absent or oversized",
    )
}

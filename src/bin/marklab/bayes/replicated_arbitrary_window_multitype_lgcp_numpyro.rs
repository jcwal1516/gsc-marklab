use std::fs;

use marklab_bayes::{sha256_hex, BackendContract};

use super::{replicated_arbitrary_window_multitype_lgcp::Prepared, run_worker, BayesCliError};

const NUMPYRO_VERSION: &str = "0.21.0";
pub(crate) const JAX_VERSION: &str = "0.11.1";
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
) -> Result<Execution, BayesCliError> {
    if !(10..=14).contains(&depth) {
        return Err(BayesCliError::Input(
            "NumPyro replicated multitype LGCP depth is invalid".into(),
        ));
    }
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workers = repository.join("workers/python");
    let worker_name = "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_worker.py";
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read_bounded(&workers.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_bounded(&workers.join(worker_name))?),
    };
    let request = serde_json::json!({
        "format": "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_request",
        "version": 1,
        "backend": backend,
        "jax_version": JAX_VERSION,
        "source_request_sha256": prepared.request_sha256(),
        "source_request": serde_json::from_slice::<serde_json::Value>(prepared.request_bytes())?,
        "maximum_tree_depth": depth,
    });
    let bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&bytes);
    let output = run_worker(repository, worker_name, &bytes, timeout_seconds)?;
    Ok(Execution {
        result: serde_json::from_slice(&output)?,
        request_sha256,
        backend,
    })
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_ADAPTER_BYTES {
        return Err(BayesCliError::Input(
            "NumPyro replicated multitype LGCP adapter is absent or oversized".into(),
        ));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

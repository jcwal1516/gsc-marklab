use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::worker_process::{self, WorkerProcessError};

use serde::Serialize;

use super::{
    super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError},
    CliError,
};

/// SHA-256 digests of the worker environment lock and of one worker script, which every
/// request binds so a result can be traced to the exact backend that produced it.
pub(crate) struct WorkerIdentity {
    pub(crate) environment_lock_sha256: String,
    pub(crate) worker_sha256: String,
}

pub(crate) fn worker_identity(
    repository: &Path,
    worker_file_name: &str,
) -> Result<WorkerIdentity, CliError> {
    let directory = repository.join("workers/python");
    let digest = |path: PathBuf| {
        fs::read(&path)
            .map(|bytes| marklab_bayes::sha256_hex(&bytes))
            .map_err(|source| CliError::Io { path, source })
    };
    Ok(WorkerIdentity {
        environment_lock_sha256: digest(directory.join("uv.lock"))?,
        worker_sha256: digest(directory.join(worker_file_name))?,
    })
}

pub(crate) fn run_worker(
    repository: &Path,
    worker_file_name: &str,
    request: &[u8],
    timeout_seconds: u64,
) -> Result<Vec<u8>, CliError> {
    if std::env::var_os("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION").is_some() {
        return Err(CliError::Backend(
            "external backend execution is disabled by MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"
                .into(),
        ));
    }
    let interpreter = marklab::backend::python_backend_interpreter(repository)?;
    let worker = repository.join("workers/python").join(worker_file_name);
    if !worker.is_file() {
        return Err(CliError::Backend(format!(
            "static worker is missing at {}",
            worker.display()
        )));
    }
    let cache = marklab::backend::python_backend_cache(repository)?;
    fs::create_dir_all(&cache).map_err(|source| CliError::Io {
        path: cache.clone(),
        source,
    })?;
    let mut command = marklab::backend::python_backend_command(&interpreter, &worker)?;
    if worker_file_name == "marklab_scipy_projected_variograms_worker.py" {
        let executable = std::env::current_exe().map_err(|source| CliError::Io {
            path: "current executable".into(),
            source,
        })?;
        command.env("MARKLAB_PROJECTED_VARIOGRAM_EXECUTABLE", executable);
    }
    command.env(
        "PYTENSOR_FLAGS",
        format!("base_compiledir={}", cache.display()),
    );
    worker_process::run_worker(
        &mut command,
        request,
        timeout_seconds,
        "external backend worker",
    )
    .map_err(|error| match error {
        WorkerProcessError::Io { path, source } => CliError::Io { path, source },
        WorkerProcessError::Backend(message) => CliError::Backend(message),
    })
}

pub(crate) fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CliError> {
    publish_pretty_json(path, result, "bayes").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            CliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            CliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => CliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => CliError::Json(error),
    })
}

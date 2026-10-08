use std::{fs, path::Path};

use crate::worker_process::{self, WorkerProcessError};

use serde::Serialize;

use super::{
    super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError},
    BayesCliError,
};

pub(crate) fn run_worker(
    repository: &Path,
    worker_file_name: &str,
    request: &[u8],
    timeout_seconds: u64,
) -> Result<Vec<u8>, BayesCliError> {
    if std::env::var_os("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION").is_some() {
        return Err(BayesCliError::Backend(
            "external backend execution is disabled by MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"
                .into(),
        ));
    }
    let interpreter = marklab::python_backend_interpreter(repository)?;
    let worker = repository.join("workers/python").join(worker_file_name);
    if !worker.is_file() {
        return Err(BayesCliError::Backend(format!(
            "static worker is missing at {}",
            worker.display()
        )));
    }
    let cache = marklab::python_backend_cache(repository)?;
    fs::create_dir_all(&cache).map_err(|source| BayesCliError::Io {
        path: cache.clone(),
        source,
    })?;
    let mut command = marklab::python_backend_command(&interpreter, &worker);
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
        WorkerProcessError::Io { path, source } => BayesCliError::Io { path, source },
        WorkerProcessError::Backend(message) => BayesCliError::Backend(message),
    })
}

pub(crate) fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), BayesCliError> {
    publish_pretty_json(path, result, "bayes").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            BayesCliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            BayesCliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => BayesCliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => BayesCliError::Json(error),
    })
}

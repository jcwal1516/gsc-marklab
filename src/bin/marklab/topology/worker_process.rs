use std::path::Path;

use crate::worker_process::{self, WorkerProcessError};

use super::CliError;

pub(crate) fn run_worker(
    repository: &Path,
    worker: &Path,
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
    let mut command = marklab::backend::python_backend_command(&interpreter, worker)?;
    command
        .env("JAX_PLATFORMS", "cpu")
        .env("XLA_FLAGS", "--xla_cpu_multi_thread_eigen=false");
    worker_process::run_worker(&mut command, request, timeout_seconds, "GUDHI worker").map_err(
        |error| match error {
            WorkerProcessError::Io { path, source } => CliError::Io { path, source },
            WorkerProcessError::Backend(message) => CliError::Backend(message),
        },
    )
}

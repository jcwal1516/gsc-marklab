use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::ExitStatus,
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use super::{
    super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError},
    BayesCliError,
};

const MAXIMUM_WORKER_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

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
    let mut child = marklab::python_backend_command(&interpreter, &worker)
        .env(
            "PYTENSOR_FLAGS",
            format!("base_compiledir={}", cache.display()),
        )
        .spawn()
        .map_err(|source| BayesCliError::Io {
            path: interpreter.clone(),
            source,
        })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| BayesCliError::Backend("worker stdout was not piped".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| BayesCliError::Backend("worker stderr was not piped".into()))?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout, MAXIMUM_WORKER_OUTPUT_BYTES));
    let stderr_reader = thread::spawn(move || read_bounded(stderr, MAXIMUM_WORKER_OUTPUT_BYTES));
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        let _ = stdout_reader.join();
        let _ = stderr_reader.join();
        return Err(BayesCliError::Backend("worker stdin was not piped".into()));
    };
    let write_result = stdin.write_all(request);
    drop(stdin);
    if let Err(source) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        let _ = stdout_reader.join();
        let _ = stderr_reader.join();
        return Err(BayesCliError::Io {
            path: worker.clone(),
            source,
        });
    }

    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|source| BayesCliError::Io {
            path: worker.clone(),
            source,
        })? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(BayesCliError::Backend(format!(
                "external backend worker exceeded the {timeout_seconds}-second limit"
            )));
        }
        thread::sleep(Duration::from_millis(25));
    };

    let (stdout, stdout_exceeded) = join_reader(stdout_reader, "stdout")?;
    let (stderr, stderr_exceeded) = join_reader(stderr_reader, "stderr")?;
    validate_process_result(status, stdout, stdout_exceeded, stderr, stderr_exceeded)
}

fn read_bounded(mut reader: impl Read, limit: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::new();
    let mut exceeded = false;
    let mut chunk = [0_u8; 8_192];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let available = limit.saturating_sub(retained.len());
        retained.extend_from_slice(&chunk[..count.min(available)]);
        exceeded |= count > available;
    }
    Ok((retained, exceeded))
}

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
    stream: &str,
) -> Result<(Vec<u8>, bool), BayesCliError> {
    reader
        .join()
        .map_err(|_| BayesCliError::Backend(format!("worker {stream} reader panicked")))?
        .map_err(|source| BayesCliError::Io {
            path: PathBuf::from(format!("external backend worker {stream}")),
            source,
        })
}

fn validate_process_result(
    status: ExitStatus,
    stdout: Vec<u8>,
    stdout_exceeded: bool,
    stderr: Vec<u8>,
    stderr_exceeded: bool,
) -> Result<Vec<u8>, BayesCliError> {
    if stdout_exceeded || stderr_exceeded {
        return Err(BayesCliError::Backend(
            "external backend worker exceeded the 16 MiB output limit".into(),
        ));
    }
    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr);
        return Err(BayesCliError::Backend(format!(
            "external backend worker exited with {status}: {}",
            detail.trim()
        )));
    }
    if stdout.is_empty() {
        return Err(BayesCliError::Backend(
            "external backend worker succeeded without a JSON result".into(),
        ));
    }
    Ok(stdout)
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

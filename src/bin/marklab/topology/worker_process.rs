use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::TopologyCliError;

const MAXIMUM_WORKER_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn run_worker(
    repository: &Path,
    worker: &Path,
    request: &[u8],
    timeout_seconds: u64,
) -> Result<Vec<u8>, TopologyCliError> {
    if std::env::var_os("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION").is_some() {
        return Err(TopologyCliError::Backend(
            "external backend execution is disabled by MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"
                .into(),
        ));
    }
    let interpreter = marklab::python_backend_interpreter(repository)?;
    let mut child = Command::new(&interpreter)
        .arg("-I")
        .arg(worker)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .env("PYTHONHASHSEED", "0")
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("OMP_NUM_THREADS", "1")
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("MKL_NUM_THREADS", "1")
        .env("JAX_PLATFORMS", "cpu")
        .env("XLA_FLAGS", "--xla_cpu_multi_thread_eigen=false")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| TopologyCliError::Io {
            path: interpreter,
            source,
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| TopologyCliError::Backend("worker stdout was not piped".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| TopologyCliError::Backend("worker stderr was not piped".into()))?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout));
    let stderr_reader = thread::spawn(move || read_bounded(stderr));
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| TopologyCliError::Backend("worker stdin was not piped".into()))?;
    stdin
        .write_all(request)
        .map_err(|source| TopologyCliError::Io {
            path: worker.to_owned(),
            source,
        })?;
    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|source| TopologyCliError::Io {
            path: worker.to_owned(),
            source,
        })? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(TopologyCliError::Backend(format!(
                "GUDHI worker exceeded the {timeout_seconds}-second limit"
            )));
        }
        thread::sleep(Duration::from_millis(25));
    };
    let (stdout, stdout_exceeded) = join_reader(stdout_reader, "stdout")?;
    let (stderr, stderr_exceeded) = join_reader(stderr_reader, "stderr")?;
    validate_process_result(status, stdout, stdout_exceeded, stderr, stderr_exceeded)
}

fn read_bounded(mut reader: impl Read) -> std::io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::new();
    let mut exceeded = false;
    let mut chunk = [0_u8; 8_192];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let available = MAXIMUM_WORKER_OUTPUT_BYTES.saturating_sub(retained.len());
        retained.extend_from_slice(&chunk[..count.min(available)]);
        exceeded |= count > available;
    }
    Ok((retained, exceeded))
}

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
    stream: &str,
) -> Result<(Vec<u8>, bool), TopologyCliError> {
    reader
        .join()
        .map_err(|_| TopologyCliError::Backend(format!("worker {stream} reader panicked")))?
        .map_err(|source| TopologyCliError::Io {
            path: PathBuf::from(format!("topology worker {stream}")),
            source,
        })
}

fn validate_process_result(
    status: ExitStatus,
    stdout: Vec<u8>,
    stdout_exceeded: bool,
    stderr: Vec<u8>,
    stderr_exceeded: bool,
) -> Result<Vec<u8>, TopologyCliError> {
    if stdout_exceeded || stderr_exceeded {
        return Err(TopologyCliError::Backend(
            "GUDHI worker exceeded the 16 MiB output limit".into(),
        ));
    }
    if !status.success() {
        return Err(TopologyCliError::Backend(format!(
            "GUDHI worker exited with {status}: {}",
            String::from_utf8_lossy(&stderr).trim()
        )));
    }
    if stdout.is_empty() {
        return Err(TopologyCliError::Backend(
            "GUDHI worker succeeded without a JSON result".into(),
        ));
    }
    Ok(stdout)
}

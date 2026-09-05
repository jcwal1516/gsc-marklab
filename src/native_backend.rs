//! Killable native scientific execution. Mathematical owners never launch processes.
use std::{
    io::{self, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

const OUTPUT_LIMIT: u64 = 16 * 1024 * 1024;

/// A native scientific process could not complete within its declared contract.
#[derive(Debug, Error)]
pub enum NativeBackendError {
    #[error("native worker I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("native worker failed: {0}")]
    Failed(String),
}

/// Execute the registered native conformal worker in this same CLI binary.
///
/// The timeout covers request writing, computation and result reading. Streams are bounded to
/// 16 MiB each; failed or timed-out processes are killed and reaped. No Python or ambient runtime
/// configuration is consumed. This entry point is for the CLI application; library clients call
/// the scientific fit directly with its cooperative deadline.
#[doc(hidden)]
pub fn run_native_grouped_conformal(
    request: Vec<u8>,
    timeout_seconds: u64,
) -> Result<Vec<u8>, NativeBackendError> {
    run_registered("native-grouped-conformal", request, timeout_seconds)
}

/// Execute native calibration with the same bounded streams and hard deadline as conformal.
#[doc(hidden)]
pub fn run_native_prediction_calibration(
    request: Vec<u8>,
    timeout_seconds: u64,
) -> Result<Vec<u8>, NativeBackendError> {
    run_registered("native-prediction-calibration", request, timeout_seconds)
}

/// Execute native late fusion with bounded streams and a hard process deadline.
#[doc(hidden)]
pub fn run_native_late_fusion(
    request: Vec<u8>,
    timeout_seconds: u64,
) -> Result<Vec<u8>, NativeBackendError> {
    run_registered("native-late-fusion", request, timeout_seconds)
}

fn run_registered(
    route: &str,
    request: Vec<u8>,
    timeout_seconds: u64,
) -> Result<Vec<u8>, NativeBackendError> {
    if !(1..=3600).contains(&timeout_seconds) {
        return Err(NativeBackendError::Failed(
            "timeout outside 1..=3600 seconds".into(),
        ));
    }
    let binary: PathBuf = std::env::current_exe()?;
    let mut command = Command::new(binary);
    command.args(["backend", route]).env_clear();
    run(command, request, Duration::from_secs(timeout_seconds))
}

struct ReapedChild(Child);
impl Drop for ReapedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run(
    mut command: Command,
    request: Vec<u8>,
    timeout: Duration,
) -> Result<Vec<u8>, NativeBackendError> {
    let deadline = Instant::now() + timeout;
    let mut child = ReapedChild(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let mut stdin = child
        .0
        .stdin
        .take()
        .ok_or_else(|| NativeBackendError::Failed("stdin missing".into()))?;
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or_else(|| NativeBackendError::Failed("stdout missing".into()))?;
    let stderr = child
        .0
        .stderr
        .take()
        .ok_or_else(|| NativeBackendError::Failed("stderr missing".into()))?;
    let writer = thread::spawn(move || stdin.write_all(&request));
    let reader = thread::spawn(move || read_bounded(stdout));
    let errors = thread::spawn(move || read_bounded(stderr));
    let status = loop {
        match child.0.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Err(error) => break Err(NativeBackendError::Io(error)),
            Ok(None) => {}
        }
        if Instant::now() >= deadline {
            break Err(NativeBackendError::Failed(
                "hard process deadline exceeded".into(),
            ));
        }
        thread::sleep(Duration::from_millis(2));
    };
    // Close all process-side pipes before joining the I/O threads on every failure path.
    drop(child);
    let written = writer
        .join()
        .map_err(|_| NativeBackendError::Failed("stdin thread panicked".into()));
    let output = reader
        .join()
        .map_err(|_| NativeBackendError::Failed("stdout thread panicked".into()));
    let errors = errors
        .join()
        .map_err(|_| NativeBackendError::Failed("stderr thread panicked".into()));
    let status = status?;
    let errors = errors??;
    if !status.success() {
        return Err(NativeBackendError::Failed(format!(
            "{status}: {}",
            String::from_utf8_lossy(&errors).trim()
        )));
    }
    written??;
    let output = output??;
    if output.is_empty() {
        return Err(NativeBackendError::Failed("empty result".into()));
    }
    Ok(output)
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>, NativeBackendError> {
    let mut bytes = Vec::new();
    reader.take(OUTPUT_LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > OUTPUT_LIMIT {
        return Err(NativeBackendError::Failed("stream exceeds 16 MiB".into()));
    }
    Ok(bytes)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn native_deadline_covers_a_blocked_stdin_writer() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exec sleep 5"]);
        let start = Instant::now();
        let error = run(command, vec![0; 1024 * 1024], Duration::from_millis(40)).unwrap_err();
        assert!(error.to_string().contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn native_nonzero_exit_preserves_stderr() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf 'fit failed' >&2; exit 7"]);
        let error = run(command, vec![], Duration::from_secs(1)).unwrap_err();
        assert!(error.to_string().contains("fit failed"));
    }
    #[test]
    fn native_stream_limit_is_exact() {
        assert_eq!(
            read_bounded(io::repeat(0).take(OUTPUT_LIMIT))
                .unwrap()
                .len() as u64,
            OUTPUT_LIMIT
        );
        assert!(read_bounded(io::repeat(0).take(OUTPUT_LIMIT + 1)).is_err());
    }
}

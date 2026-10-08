//! Bounded process transport shared by the Bayesian and topology adapters.

use std::{
    io::{self, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAXIMUM_WORKER_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub(crate) enum WorkerProcessError {
    Io { path: PathBuf, source: io::Error },
    Backend(String),
}

pub(crate) fn run_worker(
    command: &mut Command,
    request: &[u8],
    timeout_seconds: u64,
    label: &str,
) -> Result<Vec<u8>, WorkerProcessError> {
    let started = Instant::now();
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| WorkerProcessError::Io {
            path: PathBuf::from(command.get_program()),
            source,
        })?;
    let mut stdin = child.stdin.take().expect("stdin piped by this runner");
    let stdout = child.stdout.take().expect("stdout piped by this runner");
    let stderr = child.stderr.take().expect("stderr piped by this runner");

    thread::scope(|scope| {
        // A worker may stop reading stdin. Monitor its lifetime while sending the request.
        let writer = scope.spawn(move || stdin.write_all(request));
        let stdout_reader = scope.spawn(move || read_bounded(stdout));
        let stderr_reader = scope.spawn(move || read_bounded(stderr));
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Err(source) => {
                    break Err(WorkerProcessError::Io {
                        path: PathBuf::from(label),
                        source,
                    });
                }
                Ok(None) => {}
            }
            if started.elapsed() >= Duration::from_secs(timeout_seconds) {
                break Err(WorkerProcessError::Backend(format!(
                    "{label} exceeded the {timeout_seconds}-second limit"
                )));
            }
            thread::sleep(Duration::from_millis(25));
        };
        if status.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Reap the child and join every pipe on both the success and failure paths.
        let written = writer.join();
        let stdout = stdout_reader.join();
        let stderr = stderr_reader.join();
        let status = status?;
        let (stdout, stdout_exceeded) = stream_result(stdout, label, "stdout")?;
        let (stderr, stderr_exceeded) = stream_result(stderr, label, "stderr")?;
        if stdout_exceeded || stderr_exceeded {
            return Err(WorkerProcessError::Backend(format!(
                "{label} exceeded the 16 MiB output limit"
            )));
        }
        if !status.success() {
            return Err(WorkerProcessError::Backend(format!(
                "{label} exited with {status}: {}",
                String::from_utf8_lossy(&stderr).trim()
            )));
        }
        stream_result(written, label, "stdin")?;
        if stdout.is_empty() {
            return Err(WorkerProcessError::Backend(format!(
                "{label} succeeded without a JSON result"
            )));
        }
        Ok(stdout)
    })
}

fn read_bounded(mut reader: impl Read) -> io::Result<(Vec<u8>, bool)> {
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

fn stream_result<T>(
    result: thread::Result<io::Result<T>>,
    label: &str,
    stream: &str,
) -> Result<T, WorkerProcessError> {
    result
        .map_err(|_| WorkerProcessError::Backend(format!("{label} {stream} thread panicked")))?
        .map_err(|source| WorkerProcessError::Io {
            path: PathBuf::from(format!("{label} {stream}")),
            source,
        })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    #[test]
    fn worker_timeout_includes_sending_the_request() {
        let directory = tempfile::tempdir().unwrap();
        let worker = directory.path().join("slow_reader.py");
        fs::write(
            &worker,
            "import sys, time\ntime.sleep(3)\nsys.stdin.buffer.read()\nprint('{}')\n",
        )
        .unwrap();
        let interpreter =
            marklab::python_backend_interpreter(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let mut command = marklab::python_backend_command(&interpreter, &worker);
        let request = vec![b' '; 1024 * 1024];
        let started = Instant::now();
        let error = run_worker(&mut command, &request, 1, "test worker").unwrap_err();
        assert!(matches!(error, WorkerProcessError::Backend(message)
            if message.contains("exceeded the 1-second limit")));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn worker_transport_preserves_bytes_and_reports_output_and_exit_failures() {
        let directory = tempfile::tempdir().unwrap();
        let worker = directory.path().join("worker.py");
        let interpreter =
            marklab::python_backend_interpreter(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let request = vec![b'x'; 1024 * 1024];
        for (script, expected_error) in [
            ("sys.stdout.buffer.write(sys.stdin.buffer.read())", None),
            (
                "sys.stdin.buffer.read(); sys.stdout.write('x' * (16 * 1024 * 1024 + 1))",
                Some("16 MiB output limit"),
            ),
            (
                "sys.stderr.write('worker failure'); sys.exit(2)",
                Some("worker failure"),
            ),
        ] {
            fs::write(&worker, format!("import sys\n{script}\n")).unwrap();
            let mut command = marklab::python_backend_command(&interpreter, &worker);
            let result = run_worker(&mut command, &request, 10, "test worker");
            if let Some(expected) = expected_error {
                assert!(matches!(result, Err(WorkerProcessError::Backend(message))
                    if message.contains(expected)));
            } else {
                assert_eq!(result.unwrap(), request);
            }
        }
    }
}

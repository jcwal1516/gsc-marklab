//! Installed Python asset and environment locations shared by existing backend adapters.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use thiserror::Error;

/// Executable search path for workers when `MARKLAB_BACKEND_PATH` is unset.
const DEFAULT_WORKER_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// Build the curated startup policy shared by the static scientific worker runners.
///
/// The callers own interpreter/worker admission, special backend caches, stream limits and child
/// lifetime. No ambient environment is inherited. The returned expert builder must not be given
/// additional import paths or untrusted environment variables by an application adapter.
///
/// Workers search the system directories for executables such as PyTensor's C++ compiler. Set
/// `MARKLAB_BACKEND_PATH` to a list of absolute directories to use instead, for example on Nix
/// or with a conda-provided compiler.
pub fn python_backend_command(
    interpreter: &Path,
    worker: &Path,
) -> Result<Command, PythonBackendRuntimeError> {
    let search_path = worker_search_path()?;
    let mut command = Command::new(interpreter);
    // -I also implies -E, which ignores the deliberately curated PYTHONHASHSEED.
    // Keep its import protections explicitly and admit only the environment below.
    command
        .args(["-P", "-s", "-B"])
        .arg(worker)
        .env_clear()
        .env("PATH", search_path)
        .env("LC_ALL", "C")
        .env("PYTHONHASHSEED", "0")
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("OMP_NUM_THREADS", "1")
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("MKL_NUM_THREADS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command)
}

fn worker_search_path() -> Result<std::ffi::OsString, PythonBackendRuntimeError> {
    const VARIABLE: &str = "MARKLAB_BACKEND_PATH";
    let Some(value) = env::var_os(VARIABLE) else {
        return Ok(DEFAULT_WORKER_PATH.into());
    };
    let directories = env::split_paths(&value).collect::<Vec<_>>();
    if directories.is_empty() || directories.iter().any(|path| path.as_os_str().is_empty()) {
        return Err(PythonBackendRuntimeError::EmptySearchPath { variable: VARIABLE });
    }
    if let Some(path) = directories.iter().find(|path| !path.is_absolute()) {
        return Err(PythonBackendRuntimeError::RelativeOverride {
            variable: VARIABLE,
            path: path.clone(),
        });
    }
    Ok(value)
}

/// A backend location cannot be resolved or accepted.
#[derive(Debug, Error)]
pub enum PythonBackendRuntimeError {
    /// An explicit location must be independent of the working directory.
    #[error("{variable} must name an absolute path, observed {path}")]
    RelativeOverride {
        /// Environment variable selecting the location.
        variable: &'static str,
        /// Rejected location.
        path: PathBuf,
    },
    /// An explicit search path must list at least one directory and no empty entries.
    #[error("{variable} must list absolute directories separated by the platform path separator")]
    EmptySearchPath {
        /// Environment variable holding the search path.
        variable: &'static str,
    },
    /// A required runtime file or executable cannot be accessed.
    #[error("Python backend {operation} failed at {path}: {source}; configure MARKLAB_RUNTIME_ROOT and MARKLAB_PYTHON for an installed runtime")]
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Location involved in the failure.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: io::Error,
    },
}

/// Locate the directory containing `workers/python/uv.lock` and `pyproject.toml`.
///
/// An absolute `MARKLAB_RUNTIME_ROOT` takes precedence and never falls back on error.
/// Otherwise use assets beside the executable, then the build checkout only for a binary
/// executing inside that checkout's `target` directory. This does not require a Python
/// interpreter: cached scientific output may be replayed without executing a backend.
pub fn python_backend_assets_root() -> Result<PathBuf, PythonBackendRuntimeError> {
    let root = if let Some(root) = absolute_override("MARKLAB_RUNTIME_ROOT")? {
        root
    } else {
        let executable = env::current_exe().map_err(|source| PythonBackendRuntimeError::Io {
            operation: "locate executable",
            path: PathBuf::from("current executable"),
            source,
        })?;
        let beside = executable
            .parent()
            .ok_or_else(|| PythonBackendRuntimeError::Io {
                operation: "locate asset directory",
                path: executable.clone(),
                source: io::Error::new(io::ErrorKind::InvalidInput, "executable has no parent"),
            })?;
        let bundled = beside.join("workers/python");
        let has_bundle = bundled
            .try_exists()
            .map_err(|source| PythonBackendRuntimeError::Io {
                operation: "inspect bundled assets",
                path: bundled,
                source,
            })?;
        let development = Path::new(env!("CARGO_MANIFEST_DIR"));
        if has_bundle {
            beside.to_owned()
        } else if executable.starts_with(development.join("target")) {
            development.to_owned()
        } else {
            beside.to_owned()
        }
    };
    for name in ["uv.lock", "pyproject.toml"] {
        regular_file(
            &root.join("workers/python").join(name),
            "read runtime control",
        )?;
    }
    Ok(root)
}

/// Resolve the interpreter only when backend execution is requested.
///
/// Prefer absolute `MARKLAB_PYTHON`; otherwise retain the development environment under
/// `assets_root/target/pymc-venv`, using the platform's virtual-environment layout.
/// The worker remains responsible for exact Python/package version admission.
pub fn python_backend_interpreter(
    assets_root: &Path,
) -> Result<PathBuf, PythonBackendRuntimeError> {
    let interpreter = absolute_override("MARKLAB_PYTHON")?.unwrap_or_else(|| {
        assets_root.join(if cfg!(windows) {
            "target/pymc-venv/Scripts/python.exe"
        } else {
            "target/pymc-venv/bin/python"
        })
    });
    regular_file(&interpreter, "locate interpreter")?;
    Ok(interpreter)
}

/// Resolve the writable compilation-cache location without creating it.
///
/// Prefer absolute `MARKLAB_BACKEND_CACHE`; the default keeps the development
/// cache under the runtime root. Read-only installations should set the override explicitly.
pub fn python_backend_cache(assets_root: &Path) -> Result<PathBuf, PythonBackendRuntimeError> {
    Ok(absolute_override("MARKLAB_BACKEND_CACHE")?
        .unwrap_or_else(|| assets_root.join("target/pymc-cache")))
}

fn absolute_override(variable: &'static str) -> Result<Option<PathBuf>, PythonBackendRuntimeError> {
    env::var_os(variable)
        .map(|value| {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err(PythonBackendRuntimeError::RelativeOverride { variable, path });
            }
            Ok(path)
        })
        .transpose()
}

fn regular_file(path: &Path, operation: &'static str) -> Result<(), PythonBackendRuntimeError> {
    let metadata = fs::metadata(path).map_err(|source| PythonBackendRuntimeError::Io {
        operation,
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(PythonBackendRuntimeError::Io {
            operation,
            path: path.to_owned(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "expected a regular file"),
        });
    }
    Ok(())
}

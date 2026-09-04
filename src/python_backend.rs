//! Installed Python asset and environment locations shared by existing backend adapters.

use std::{env, fs, io, path::{Path, PathBuf}};

use thiserror::Error;

/// A backend location cannot be resolved or admitted.
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
        let beside = executable.parent().ok_or_else(|| PythonBackendRuntimeError::Io {
            operation: "locate asset directory",
            path: executable.clone(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "executable has no parent"),
        })?;
        let bundled = beside.join("workers/python");
        let has_bundle = bundled.try_exists().map_err(|source| PythonBackendRuntimeError::Io {
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
        regular_file(&root.join("workers/python").join(name), "read runtime control")?;
    }
    Ok(root)
}

/// Resolve the interpreter only when backend execution is requested.
///
/// Prefer absolute `MARKLAB_PYTHON`; otherwise retain the development environment under
/// `assets_root/target/pymc-venv`, using the platform's virtual-environment layout.
/// The worker remains responsible for exact Python/package version admission.
pub fn python_backend_interpreter(assets_root: &Path) -> Result<PathBuf, PythonBackendRuntimeError> {
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
/// Prefer absolute `MARKLAB_BACKEND_CACHE`; the default preserves the existing development
/// cache under the runtime root. Read-only installations should set the override explicitly.
pub fn python_backend_cache(assets_root: &Path) -> Result<PathBuf, PythonBackendRuntimeError> {
    Ok(absolute_override("MARKLAB_BACKEND_CACHE")?
        .unwrap_or_else(|| assets_root.join("target/pymc-cache")))
}

fn absolute_override(variable: &'static str) -> Result<Option<PathBuf>, PythonBackendRuntimeError> {
    env::var_os(variable).map(|value| {
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return Err(PythonBackendRuntimeError::RelativeOverride { variable, path });
        }
        Ok(path)
    }).transpose()
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

use std::path::PathBuf;

use marklab_bayes::{BayesError, BayesError::InvalidSpec};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("invalid input: {0}")]
    Input(String),
    #[error("model failed: {0}")]
    Model(#[from] BayesError),
    #[error("backend failed: {0}")]
    Backend(String),
    #[error("cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid CSV input: {0}")]
    Csv(#[from] csv::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<marklab::backend::PythonBackendRuntimeError> for CliError {
    fn from(error: marklab::backend::PythonBackendRuntimeError) -> Self {
        Self::Backend(error.to_string())
    }
}

pub(crate) fn into_marklab_error(error: CliError) -> marklab::MarklabError {
    match error {
        CliError::Input(message) => marklab::MarklabError::Validation(message),
        CliError::Model(InvalidSpec(message)) => marklab::MarklabError::Validation(message),
        CliError::Model(error) => marklab::MarklabError::Compute(error.to_string()),
        CliError::Backend(message) => marklab::MarklabError::Compute(message),
        CliError::Io { path, source } => marklab::MarklabError::io(path, source),
        CliError::Csv(error) => marklab::MarklabError::Csv(error),
        CliError::Json(error) => marklab::MarklabError::Json(error),
    }
}

use std::path::PathBuf;

use marklab_bayes::{BayesError, BayesError::InvalidSpec};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum BayesCliError {
    #[error("Bayesian input error: {0}")]
    Input(String),
    #[error("Bayesian model failed: {0}")]
    Model(#[from] BayesError),
    #[error("Bayesian backend failed: {0}")]
    Backend(String),
    #[error("Bayesian I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Bayesian CSV boundary failed: {0}")]
    Csv(#[from] csv::Error),
    #[error("Bayesian JSON boundary failed: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<marklab::PythonBackendRuntimeError> for BayesCliError {
    fn from(error: marklab::PythonBackendRuntimeError) -> Self {
        Self::Backend(error.to_string())
    }
}

pub(crate) fn into_marklab_error(error: BayesCliError) -> marklab::MarklabError {
    match error {
        BayesCliError::Input(message) => marklab::MarklabError::Validation(message),
        BayesCliError::Model(InvalidSpec(message)) => marklab::MarklabError::Validation(message),
        BayesCliError::Model(error) => marklab::MarklabError::Compute(error.to_string()),
        BayesCliError::Backend(message) => marklab::MarklabError::Compute(message),
        BayesCliError::Io { path, source } => marklab::MarklabError::io(path, source),
        BayesCliError::Csv(error) => marklab::MarklabError::Csv(error),
        BayesCliError::Json(error) => marklab::MarklabError::Json(error),
    }
}

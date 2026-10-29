use std::path::PathBuf;

use thiserror::Error;

/// Errors from reading or preprocessing a mass spectrum.
#[derive(Debug, Error)]
pub enum SpectrumError {
    /// A file could not be read.
    #[error("{path}: {source}")]
    Io {
        /// File that failed.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// A Bruker flex acquisition is malformed or uses an unsupported calibration or mode.
    #[error("Bruker flex acquisition: {0}")]
    Bruker(String),
    /// A text spectrum is malformed.
    #[error("text spectrum: {0}")]
    Text(String),
    /// Spectrum values violate the m/z or intensity contract.
    #[error("invalid spectrum: {0}")]
    Spectrum(String),
    /// A preprocessing step cannot be applied.
    #[error("preprocessing step {index} ({step}): {message}")]
    Step {
        /// Zero-based position of the step in the declared list.
        index: usize,
        /// Step name as written in the configuration.
        step: &'static str,
        /// Reason the step failed.
        message: String,
    },
    /// The binning declaration is invalid.
    #[error("binning: {0}")]
    Binning(String),
}

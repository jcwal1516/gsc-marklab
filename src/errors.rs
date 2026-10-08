use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, MarklabError>;

#[derive(Debug, Error)]
pub enum MarklabError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("input schema error: {0}")]
    Schema(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("geometry error: {0}")]
    Geometry(String),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("command I/O failed: {0}")]
    CommandIo(#[from] std::io::Error),
    #[error("JSON serialization or parsing failed: {0}")]
    Json(#[from] serde_json::Error),
    #[cfg(feature = "csv")]
    #[error("CSV serialization or parsing failed: {0}")]
    Csv(#[from] csv::Error),
    #[cfg(feature = "wsi")]
    #[error("PNG encoding failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("compute error: {0}")]
    Compute(String),
    #[error("unsupported result format version {found}; supported version is {supported}")]
    UnsupportedFormatVersion { found: String, supported: String },
    #[error("slide error: {0}")]
    Slide(String),
    #[error("unsupported slide sample type: {0}")]
    UnsupportedSlideSampleType(String),
}

impl MarklabError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

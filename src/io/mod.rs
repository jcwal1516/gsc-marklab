#[cfg(feature = "csv")]
pub mod csv;
pub mod geojson;
#[cfg(feature = "cli")]
pub mod intermediates;
#[cfg(feature = "parquet")]
pub mod parquet;
#[cfg(any(feature = "csv", feature = "parquet"))]
mod pattern_builder;
pub mod report;
#[cfg(any(feature = "csv", feature = "parquet"))]
mod row;

#[cfg(all(test, feature = "parquet"))]
mod parquet_tests;

use std::{path::Path, time::Duration};

use crate::{
    data::Pattern,
    errors::{MarklabError, Result},
    geom::mask::TumorMask,
};

/// Timings for format decoding/filtering and indexed geometry finalization.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PatternLoadDiagnostics {
    /// Time spent decoding physical rows, validating/filtering them, and building retained arrays.
    pub decode_and_filter: Duration,
    /// Time spent building the spatial index and evaluating mean nearest-neighbor distance.
    pub nearest_neighbor: Duration,
}

/// A validated domain pattern together with input-adapter timings.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternLoadResult {
    pub pattern: Pattern,
    pub diagnostics: PatternLoadDiagnostics,
}

/// Filesystem input adapter that builds validated [`Pattern`] values against one tumor mask.
#[derive(Clone, Copy, Debug)]
pub struct PatternLoader<'mask> {
    mask: &'mask TumorMask,
}

impl<'mask> PatternLoader<'mask> {
    /// Bind subsequent cell-table loads to `mask`.
    pub fn new(mask: &'mask TumorMask) -> Self {
        Self { mask }
    }

    /// Decode and validate a `.csv` or `.parquet` cell table.
    pub fn load(&self, path: impl AsRef<Path>) -> Result<Pattern> {
        Ok(self.load_with_diagnostics(path)?.pattern)
    }

    /// Decode a cell table while retaining adapter-stage timings.
    pub fn load_with_diagnostics(&self, path: impl AsRef<Path>) -> Result<PatternLoadResult> {
        let path = path.as_ref();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);

        match extension.as_deref() {
            Some("csv") => load_csv_path(path, self.mask),
            Some("parquet") => load_parquet_path(path, self.mask),
            _ => Err(MarklabError::Schema(
                "cell input extension must be .parquet or .csv".into(),
            )),
        }
    }
}

#[cfg(any(feature = "csv", feature = "parquet"))]
pub(crate) fn checked_probability(value: f32, column: &str) -> Result<f32> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(MarklabError::Schema(format!(
            "{column} must be finite and in [0, 1]"
        )))
    }
}

#[cfg(any(feature = "csv", feature = "parquet"))]
pub(crate) fn checked_positive(value: f32, column: &str) -> Result<f32> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(MarklabError::Schema(format!(
            "{column} must be finite and greater than 0"
        )))
    }
}

#[cfg(any(feature = "csv", feature = "parquet"))]
pub(crate) fn checked_finite(value: f32, column: &str) -> Result<f32> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(MarklabError::Schema(format!("{column} must be finite")))
    }
}

#[cfg(feature = "csv")]
fn load_csv_path(path: &Path, mask: &TumorMask) -> Result<PatternLoadResult> {
    csv::load_pattern_csv_with_diagnostics(path, mask)
}

#[cfg(not(feature = "csv"))]
fn load_csv_path(_path: &Path, _mask: &TumorMask) -> Result<PatternLoadResult> {
    Err(MarklabError::Schema(
        "CSV input support is disabled; enable the csv feature".into(),
    ))
}

#[cfg(feature = "parquet")]
fn load_parquet_path(path: &Path, mask: &TumorMask) -> Result<PatternLoadResult> {
    parquet::load_pattern_parquet_with_diagnostics(path, mask)
}

#[cfg(not(feature = "parquet"))]
fn load_parquet_path(_path: &Path, _mask: &TumorMask) -> Result<PatternLoadResult> {
    Err(MarklabError::Schema(
        "Parquet input support is disabled; enable the parquet feature".into(),
    ))
}

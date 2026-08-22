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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PatternLoadDiagnostics {
    pub mask_filter: Duration,
    pub nearest_neighbor: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatternLoadResult {
    pub pattern: Pattern,
    pub diagnostics: PatternLoadDiagnostics,
}

pub fn load_pattern_path(path: impl AsRef<Path>, mask: &TumorMask) -> Result<Pattern> {
    Ok(load_pattern_path_with_diagnostics(path, mask)?.pattern)
}

pub fn load_pattern_path_with_diagnostics(
    path: impl AsRef<Path>,
    mask: &TumorMask,
) -> Result<PatternLoadResult> {
    let path = path.as_ref();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);

    match extension.as_deref() {
        Some("csv") => load_csv_path(path, mask),
        Some("parquet") => load_parquet_path(path, mask),
        _ => Err(MarklabError::Schema(
            "cell input extension must be .parquet or .csv".into(),
        )),
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

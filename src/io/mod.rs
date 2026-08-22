#[cfg(feature = "csv")]
pub mod csv;
pub mod geojson;
#[cfg(feature = "cli")]
pub mod intermediates;
#[cfg(feature = "parquet")]
pub mod parquet;
pub mod report;

#[cfg(all(test, feature = "parquet"))]
mod parquet_tests;

#[cfg(any(feature = "csv", feature = "parquet"))]
use std::collections::BTreeMap;
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

#[derive(Debug, Default)]
#[cfg(any(feature = "csv", feature = "parquet"))]
pub(crate) struct CategoricalStratumEncoder {
    values: Vec<u32>,
    ids: BTreeMap<String, u32>,
    saw_nonmissing: bool,
}

#[cfg(any(feature = "csv", feature = "parquet"))]
impl CategoricalStratumEncoder {
    pub(crate) fn push_optional(&mut self, value: Option<&str>) {
        let normalized = value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("__missing__");
        self.saw_nonmissing |= normalized != "__missing__";
        let next_id = self.ids.len() as u32;
        let id = *self.ids.entry(normalized.to_owned()).or_insert(next_id);
        self.values.push(id);
    }

    pub(crate) fn finish(self) -> Option<Box<[u32]>> {
        self.saw_nonmissing.then(|| self.values.into_boxed_slice())
    }
}

#[derive(Debug, Default)]
#[cfg(any(feature = "csv", feature = "parquet"))]
pub(crate) struct DenseOptionalColumn<T> {
    values: Vec<T>,
    presence: Option<bool>,
}

#[derive(Clone, Copy, Debug)]
#[cfg(any(feature = "csv", feature = "parquet"))]
pub(crate) struct PatternRowQc {
    pub valid_tumor: bool,
    pub valid_ihc: bool,
    pub internal_control_valid: Option<bool>,
    pub artifact_excluded: Option<bool>,
    pub nonviable_excluded: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg(any(feature = "csv", feature = "parquet"))]
pub(crate) struct PatternBuildCounters {
    in_mask: usize,
    valid_tumor: usize,
    valid_ihc: usize,
    valid_internal_control: usize,
    artifact_excluded: usize,
    nonviable_excluded: usize,
    retained: usize,
    saw_internal_control: bool,
    saw_artifact: bool,
    saw_nonviable: bool,
}

#[cfg(any(feature = "csv", feature = "parquet"))]
impl PatternBuildCounters {
    pub(crate) fn observe(&mut self, row: PatternRowQc) -> bool {
        self.in_mask += 1;
        self.valid_tumor += usize::from(row.valid_tumor);
        self.valid_ihc += usize::from(row.valid_ihc);
        if let Some(valid) = row.internal_control_valid {
            self.saw_internal_control = true;
            self.valid_internal_control += usize::from(valid);
        }
        if let Some(excluded) = row.artifact_excluded {
            self.saw_artifact = true;
            self.artifact_excluded += usize::from(excluded);
        }
        if let Some(excluded) = row.nonviable_excluded {
            self.saw_nonviable = true;
            self.nonviable_excluded += usize::from(excluded);
        }

        let retained = row.valid_tumor
            && row.valid_ihc
            && row.internal_control_valid.unwrap_or(true)
            && !row.artifact_excluded.unwrap_or(false)
            && !row.nonviable_excluded.unwrap_or(false);
        self.retained += usize::from(retained);
        retained
    }

    pub(crate) fn validate_denominator(&self) -> Result<()> {
        if self.in_mask == 0 {
            return Err(MarklabError::Validation(
                "no cells fell inside the tumor mask; QC fractions are undefined".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn apply_to(self, pattern: &mut Pattern) -> Result<()> {
        self.validate_denominator()?;

        let fraction = |count| Some(count as f64 / self.in_mask as f64);
        pattern.valid_tumor_fraction = fraction(self.valid_tumor);
        pattern.valid_ihc_fraction = fraction(self.valid_ihc);
        pattern.internal_control_valid_fraction = self
            .saw_internal_control
            .then(|| self.valid_internal_control as f64 / self.in_mask as f64);
        pattern.artifact_excluded_fraction = self
            .saw_artifact
            .then(|| self.artifact_excluded as f64 / self.in_mask as f64);
        pattern.nonviable_excluded_fraction = self
            .saw_nonviable
            .then(|| self.nonviable_excluded as f64 / self.in_mask as f64);
        pattern.window.valid_mask_fraction = self.retained as f64 / self.in_mask as f64;
        Ok(())
    }
}

#[cfg(any(feature = "csv", feature = "parquet"))]
impl<T> DenseOptionalColumn<T> {
    pub(crate) fn push(&mut self, value: Option<T>, column: &str) -> Result<()> {
        let present = value.is_some();
        if self.presence.is_some_and(|expected| expected != present) {
            return Err(MarklabError::Schema(format!(
                "{column} must be populated for every retained row or none"
            )));
        }
        self.presence.get_or_insert(present);
        if let Some(value) = value {
            self.values.push(value);
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Option<Box<[T]>> {
        (self.presence == Some(true)).then(|| self.values.into_boxed_slice())
    }
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

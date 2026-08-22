use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    common::finite::validate_serializable_finite,
    errors::{MarklabError, Result},
};

use super::{AnalysisSection, TimingStage};

pub(super) fn write_timing_sidecar(out: &Path, timings: &[TimingStage]) -> Result<()> {
    write_json(
        out.join("timings.json"),
        &serde_json::json!({"stages": timings}),
    )
}

pub(super) fn write_available_json<T: Serialize>(
    path: PathBuf,
    section: &AnalysisSection<Vec<T>>,
) -> Result<()> {
    if section.value().is_some_and(|value| !value.is_empty()) {
        write_json(path, section)?;
    }
    Ok(())
}

pub(super) fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    validate_serializable_finite(value).map_err(|error| {
        MarklabError::Compute(format!(
            "output artifact contains invalid floating-point data: {error}"
        ))
    })?;
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    std::fs::write(&path, json).map_err(|source| MarklabError::io(&path, source))
}

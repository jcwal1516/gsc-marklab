use std::path::Path;

use marklab_cohort::PatientEffect;
use serde::Deserialize;

use super::{validate_input_file, CohortError};

#[derive(Debug, Deserialize)]
struct EffectRow {
    patient_id: String,
    effect: f64,
}

pub(super) fn read_effects(path: &Path) -> Result<Vec<PatientEffect>, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    if !headers.iter().eq(["patient_id", "effect"]) {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,effect".into(),
        ));
    }
    reader
        .deserialize::<EffectRow>()
        .map(|decoded| {
            let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(PatientEffect {
                patient_id: row.patient_id,
                effect: row.effect,
            })
        })
        .collect()
}

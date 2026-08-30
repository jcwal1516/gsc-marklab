use std::path::Path;

use serde::Serialize;

use super::super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};
use super::CohortError;

pub(super) fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CohortError> {
    publish_pretty_json(path, result, "cohort").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            CohortError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            CohortError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => CohortError::Output { path, source },
        ExclusiveJsonOutputError::Json(error) => CohortError::Json(error),
    })
}

use std::path::Path;

use serde::Serialize;

use super::{
    super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError},
    CliError,
};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn read_input(path: &Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "input must be a regular file within 16 MiB",
    )
}

pub(crate) fn read_required(path: &Path) -> Result<Vec<u8>, CliError> {
    let message = format!(
        "required backend artifact is missing, empty or too large: {}",
        path.display()
    );
    let bytes = crate::input_file::read_regular_file(path, MAXIMUM_INPUT_BYTES, &message).map_err(
        |error| match error {
            CliError::Input(message) => CliError::Backend(message),
            other => other,
        },
    )?;
    if bytes.is_empty() {
        return Err(CliError::Backend(message));
    }
    Ok(bytes)
}

pub(crate) fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CliError> {
    publish_pretty_json(path, result, "topology").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            CliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            CliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => CliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => CliError::Json(error),
    })
}

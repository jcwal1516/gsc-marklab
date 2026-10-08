use std::{fs, path::Path};

use serde::Serialize;

use super::{
    super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError},
    TopologyCliError,
};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn read_input(path: &Path) -> Result<Vec<u8>, TopologyCliError> {
    let metadata = fs::metadata(path).map_err(|source| TopologyCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(TopologyCliError::Input(
            "input must be a regular file within 16 MiB".into(),
        ));
    }
    fs::read(path).map_err(|source| TopologyCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn read_required(path: &Path) -> Result<Vec<u8>, TopologyCliError> {
    let bytes = fs::read(path).map_err(|source| TopologyCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_INPUT_BYTES as usize {
        return Err(TopologyCliError::Backend(format!(
            "required backend artifact is empty or too large: {}",
            path.display()
        )));
    }
    Ok(bytes)
}
pub(crate) fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), TopologyCliError> {
    publish_pretty_json(path, result, "topology").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            TopologyCliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            TopologyCliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => TopologyCliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => TopologyCliError::Json(error),
    })
}

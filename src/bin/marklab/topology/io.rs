use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use serde::Serialize;

use super::TopologyCliError;

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
    if fs::symlink_metadata(path).is_ok() {
        return Err(TopologyCliError::Input(format!(
            "output already exists: {}",
            path.display()
        )));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| TopologyCliError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| TopologyCliError::Input("output must name a file".into()))?;
    let mut staging_name = OsString::from(".");
    staging_name.push(file_name);
    staging_name.push(format!(".marklab-topology-{}.tmp", std::process::id()));
    let staging = parent.join(staging_name);
    let bytes = serde_json::to_vec_pretty(result)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .map_err(|source| TopologyCliError::Io {
            path: staging.clone(),
            source,
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|source| TopologyCliError::Io {
            path: staging.clone(),
            source,
        })?;
    fs::rename(&staging, path).map_err(|source| TopologyCliError::Io {
        path: path.to_owned(),
        source,
    })
}

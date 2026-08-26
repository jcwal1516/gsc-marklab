use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use serde::Serialize;

use super::CohortError;

pub(super) fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CohortError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(CohortError::Input(format!(
                "output already exists: {}",
                path.display()
            )))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CohortError::Output {
                path: path.to_owned(),
                source,
            })
        }
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| CohortError::Output {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| CohortError::Input("output must name a file".into()))?;
    let mut staging_name = OsString::from(".");
    staging_name.push(file_name);
    staging_name.push(format!(".marklab-cohort-{}.tmp", std::process::id()));
    let staging = parent.join(staging_name);
    let bytes = serde_json::to_vec_pretty(result)?;
    let publication = (|| -> Result<(), CohortError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|source| CohortError::Output {
                path: staging.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|source| CohortError::Output {
                path: staging.clone(),
                source,
            })?;
        fs::rename(&staging, path).map_err(|source| CohortError::Output {
            path: path.to_owned(),
            source,
        })
    })();
    if publication.is_err() {
        let _ = fs::remove_file(&staging);
    }
    publication
}

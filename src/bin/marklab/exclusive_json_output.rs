use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::Serialize;

#[derive(Debug)]
pub(crate) enum ExclusiveJsonOutputError {
    OutputExists,
    OutputMustNameFile,
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json(serde_json::Error),
}

pub(crate) fn publish_pretty_json(
    path: &Path,
    result: &impl Serialize,
    staging_namespace: &str,
) -> Result<(), ExclusiveJsonOutputError> {
    match fs::symlink_metadata(path) {
        Ok(_) => return Err(ExclusiveJsonOutputError::OutputExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(ExclusiveJsonOutputError::Io {
                path: path.to_owned(),
                source,
            });
        }
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| ExclusiveJsonOutputError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .ok_or(ExclusiveJsonOutputError::OutputMustNameFile)?;
    let mut staging_name = OsString::from(".");
    staging_name.push(file_name);
    staging_name.push(format!(
        ".marklab-{staging_namespace}-{}.tmp",
        std::process::id()
    ));
    let staging = parent.join(staging_name);
    let bytes = serde_json::to_vec_pretty(result).map_err(ExclusiveJsonOutputError::Json)?;

    let publication = (|| -> Result<(), ExclusiveJsonOutputError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|source| ExclusiveJsonOutputError::Io {
                path: staging.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|source| ExclusiveJsonOutputError::Io {
                path: staging.clone(),
                source,
            })?;
        fs::rename(&staging, path).map_err(|source| ExclusiveJsonOutputError::Io {
            path: path.to_owned(),
            source,
        })
    })();
    if publication.is_err() {
        let _ = fs::remove_file(&staging);
    }
    publication
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{publish_pretty_json, ExclusiveJsonOutputError};

    #[test]
    fn publishes_pretty_json_once_with_the_existing_byte_contract() {
        let directory = tempdir().expect("temporary output directory");
        let output = directory.path().join("result.json");

        publish_pretty_json(&output, &serde_json::json!({"value": [1, 2]}), "test")
            .expect("first publication");

        assert_eq!(
            std::fs::read(&output).expect("published output"),
            b"{\n  \"value\": [\n    1,\n    2\n  ]\n}\n"
        );
        assert!(matches!(
            publish_pretty_json(&output, &serde_json::json!({"value": 3}), "test"),
            Err(ExclusiveJsonOutputError::OutputExists)
        ));
    }
}

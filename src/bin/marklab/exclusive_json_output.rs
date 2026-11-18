use std::{
    io::Write,
    path::{Path, PathBuf},
};

use serde::Serialize;

use super::exclusive_file_output::{self, ExclusiveFileError};

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
    exclusive_file_output::publish_exclusive_file(path, staging_namespace, true, |file| {
        let bytes = serde_json::to_vec_pretty(result).map_err(ExclusiveJsonOutputError::Json)?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .map_err(|source| ExclusiveJsonOutputError::Io {
                path: path.to_owned(),
                source,
            })
    })
    .map_err(|error| match error {
        ExclusiveFileError::OutputExists => ExclusiveJsonOutputError::OutputExists,
        ExclusiveFileError::OutputMustNameFile => ExclusiveJsonOutputError::OutputMustNameFile,
        ExclusiveFileError::Io { path, source } => ExclusiveJsonOutputError::Io { path, source },
        ExclusiveFileError::Write(error) => error,
    })
}

#[cfg(test)]
mod tests {
    use std::{io::Write as _, path::Path};

    use serde::{Serialize, Serializer};
    use tempfile::tempdir;

    use crate::exclusive_file_output::{self, ExclusiveFileError};

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

    #[test]
    fn concurrent_publication_preserves_the_first_output() {
        struct PublishFirst<'a>(&'a Path);

        impl Serialize for PublishFirst<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                // Publish after the existence check, without a timing-dependent thread race.
                publish_pretty_json(self.0, &"first result", "test").unwrap();
                42.serialize(serializer)
            }
        }

        for topology in [false, true] {
            let directory = tempdir().unwrap();
            let output = directory.path().join("result.json");
            let result = PublishFirst(&output);
            let rejected = if topology {
                crate::topology::publish_json(&output, &result).is_err()
            } else {
                publish_pretty_json(&output, &result, "test").is_err()
            };
            assert!(rejected, "a concurrent publication must be rejected");
            assert_eq!(std::fs::read(&output).unwrap(), b"\"first result\"\n");
        }
    }

    #[test]
    fn failed_writer_does_not_publish_partial_bytes_and_can_retry() {
        let directory = tempdir().unwrap();
        let output = directory.path().join("result.json");

        assert!(matches!(
            exclusive_file_output::publish_exclusive_file(
                &output,
                "test",
                true,
                |file| -> Result<(), std::io::Error> {
                    file.write_all(b"partial")?;
                    Err(std::io::Error::other("injected writer failure"))
                }
            ),
            Err(ExclusiveFileError::Write(_))
        ));
        assert!(!output.exists());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);

        publish_pretty_json(&output, &serde_json::json!({"complete": true}), "test")
            .expect("retry after failed writer");
        assert_eq!(
            std::fs::read(output).unwrap(),
            b"{\n  \"complete\": true\n}\n"
        );
    }
}

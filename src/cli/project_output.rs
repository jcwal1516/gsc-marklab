use std::{io::Write, path::Path};

use crate::{output::publish_exclusive_file, workflow::CacheStatus, MarklabError, Result};

pub(super) fn write_output(
    path: &Path,
    encoded: &[u8],
    cache_status: CacheStatus,
    command: &str,
) -> Result<()> {
    publish_exclusive_file(path, "project-output", false, |output| {
        output
            .write_all(encoded)
            .map_err(|source| MarklabError::io(path, source))
    })?;
    let cache = match cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project {command} cache_status={cache}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use marklab_workflow::CacheStatus;

    use super::write_output;

    #[test]
    fn output_bytes_are_exact_and_existing_output_is_preserved() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = directory.path().join("result.json");

        write_output(&output, b"exact bytes", CacheStatus::Miss, "test").expect("first output");
        assert_eq!(
            std::fs::read(&output).expect("published output"),
            b"exact bytes"
        );

        assert!(write_output(&output, b"replacement", CacheStatus::Hit, "test").is_err());
        assert_eq!(
            std::fs::read(&output).expect("preserved output"),
            b"exact bytes"
        );
    }

    #[test]
    fn missing_parent_is_not_created() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let parent = directory.path().join("missing");
        let output = parent.join("result.json");

        assert!(write_output(&output, b"bytes", CacheStatus::Miss, "test").is_err());
        assert!(!parent.exists());
    }
}

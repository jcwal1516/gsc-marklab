//! Bounded reads of user-supplied input files.

use std::{fs, io::Read, path::Path};

use crate::cli_error::CliError;

/// Fail unless `path` names a regular file of at most `maximum_bytes`.
pub(crate) fn validate_regular_file(
    path: &Path,
    maximum_bytes: u64,
    invalid_message: &str,
) -> Result<(), CliError> {
    let metadata = fs::metadata(path).map_err(|source| io_error(path, source))?;
    if !metadata.is_file() || metadata.len() > maximum_bytes {
        return Err(CliError::Input(invalid_message.into()));
    }
    Ok(())
}

/// Read a regular file of at most `maximum_bytes`.
///
/// Special files are rejected before opening, the opened handle is checked again, and the
/// read itself is capped, so a file that grows after the check cannot exceed the limit.
pub(crate) fn read_regular_file(
    path: &Path,
    maximum_bytes: u64,
    invalid_message: &str,
) -> Result<Vec<u8>, CliError> {
    validate_regular_file(path, maximum_bytes, invalid_message)?;
    let file = fs::File::open(path).map_err(|source| io_error(path, source))?;
    let metadata = file.metadata().map_err(|source| io_error(path, source))?;
    if !metadata.is_file() || metadata.len() > maximum_bytes {
        return Err(CliError::Input(invalid_message.into()));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| io_error(path, source))?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(CliError::Input(invalid_message.into()));
    }
    Ok(bytes)
}

/// Read a file that ships with Marklab, such as a worker script or the environment lock.
pub(crate) fn read_file(path: &Path) -> Result<Vec<u8>, CliError> {
    fs::read(path).map_err(|source| io_error(path, source))
}

fn io_error(path: &Path, source: std::io::Error) -> CliError {
    CliError::Io {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_file_validation_preserves_limit_and_error_precedence() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let file = directory.path().join("input.bin");
        fs::write(&file, b"abc").expect("fixture");

        assert_eq!(
            read_regular_file(&file, 3, "invalid fixture").expect("bounded file"),
            b"abc"
        );
        assert!(matches!(
            validate_regular_file(&file, 2, "invalid fixture"),
            Err(CliError::Input(message)) if message == "invalid fixture"
        ));
        assert!(matches!(
            read_regular_file(&file, 2, "invalid fixture"),
            Err(CliError::Input(message)) if message == "invalid fixture"
        ));
        assert!(matches!(
            validate_regular_file(directory.path(), 3, "invalid fixture"),
            Err(CliError::Input(message)) if message == "invalid fixture"
        ));
        assert!(matches!(
            validate_regular_file(&directory.path().join("missing"), 3, "invalid fixture"),
            Err(CliError::Io { .. })
        ));
    }
}

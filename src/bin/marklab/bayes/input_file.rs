use std::{fs, io::Read, path::Path};

use super::BayesCliError;

pub(super) fn validate_regular_file(
    path: &Path,
    maximum_bytes: u64,
    invalid_message: &str,
) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > maximum_bytes {
        return Err(BayesCliError::Input(invalid_message.into()));
    }
    Ok(())
}

pub(crate) fn read_regular_file(
    path: &Path,
    maximum_bytes: u64,
    invalid_message: &str,
) -> Result<Vec<u8>, BayesCliError> {
    // Reject special files before opening; still check the opened handle and cap the read.
    validate_regular_file(path, maximum_bytes, invalid_message)?;
    let file = fs::File::open(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let metadata = file.metadata().map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > maximum_bytes {
        return Err(BayesCliError::Input(invalid_message.into()));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| BayesCliError::Io {
            path: path.to_owned(),
            source,
        })?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(BayesCliError::Input(invalid_message.into()));
    }
    Ok(bytes)
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
            Err(BayesCliError::Input(message)) if message == "invalid fixture"
        ));
        assert!(matches!(
            read_regular_file(&file, 2, "invalid fixture"),
            Err(BayesCliError::Input(message)) if message == "invalid fixture"
        ));
        assert!(matches!(
            validate_regular_file(directory.path(), 3, "invalid fixture"),
            Err(BayesCliError::Input(message)) if message == "invalid fixture"
        ));
        assert!(matches!(
            validate_regular_file(&directory.path().join("missing"), 3, "invalid fixture"),
            Err(BayesCliError::Io { .. })
        ));
    }
}

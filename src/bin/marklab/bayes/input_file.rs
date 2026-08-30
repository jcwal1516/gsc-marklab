use std::{fs, path::Path};

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

pub(super) fn read_regular_file(
    path: &Path,
    maximum_bytes: u64,
    invalid_message: &str,
) -> Result<Vec<u8>, BayesCliError> {
    validate_regular_file(path, maximum_bytes, invalid_message)?;
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
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
            validate_regular_file(directory.path(), 3, "invalid fixture"),
            Err(BayesCliError::Input(message)) if message == "invalid fixture"
        ));
        assert!(matches!(
            validate_regular_file(&directory.path().join("missing"), 3, "invalid fixture"),
            Err(BayesCliError::Io { .. })
        ));
    }
}

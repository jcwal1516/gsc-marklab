use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use crate::MicrobioError;

/// Read a regular file of at most `maximum_bytes`. The file type is checked before opening, so a
/// FIFO without a writer cannot block, and again on the open handle.
pub(crate) fn read_capped(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, MicrobioError> {
    let error = |source| MicrobioError::Io {
        path: PathBuf::from(path),
        source,
    };
    let invalid = |message: String| error(io::Error::new(io::ErrorKind::InvalidInput, message));
    if !fs::metadata(path).map_err(error)?.is_file() {
        return Err(invalid("not a regular file".to_owned()));
    }
    let file = fs::File::open(path).map_err(error)?;
    let metadata = file.metadata().map_err(error)?;
    if !metadata.is_file() {
        return Err(invalid("not a regular file".to_owned()));
    }
    if metadata.len() > maximum_bytes {
        return Err(invalid(format!("larger than {maximum_bytes} bytes")));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(invalid(format!(
            "grew beyond {maximum_bytes} bytes while it was read"
        )));
    }
    Ok(bytes)
}

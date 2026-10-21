use crate::columnar::{ColumnarError, ParquetFailure};

use super::parquet_failure;
pub(super) use crate::columnar::parquet::levels::LevelPattern;
use crate::columnar::parquet::levels::{check_level_stream, LevelStreamError};

pub(super) fn validate_level_stream(
    bytes: &[u8],
    expected_values: usize,
    pattern: LevelPattern,
) -> Result<(), ColumnarError> {
    check_level_stream(bytes, expected_values, |index| pattern.level(index)).map_err(level_error)
}

/// The crate error for a rejected level stream.
pub(in crate::columnar::parquet) fn level_error(error: LevelStreamError) -> ColumnarError {
    match error {
        LevelStreamError::Invalid => parquet_failure(ParquetFailure::InvalidPage),
        LevelStreamError::Overflow => ColumnarError::SizeOverflow,
    }
}

pub(super) fn validate_plain_byte_arrays(
    bytes: &[u8],
    expected_values: usize,
) -> Result<usize, ColumnarError> {
    let mut cursor = 0_usize;
    let mut payload_bytes = 0_usize;
    for _ in 0..expected_values {
        let length_end = cursor
            .checked_add(size_of::<u32>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let length = u32::from_le_bytes(
            bytes
                .get(cursor..length_end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?,
        );
        payload_bytes = payload_bytes
            .checked_add(usize::try_from(length).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)?;
        cursor = length_end
            .checked_add(usize::try_from(length).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)?;
        if cursor > bytes.len() {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
    }
    if cursor != bytes.len() {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(payload_bytes)
}

use crate::columnar::{EmbeddingColumnarError, ParquetFailure};

use super::parquet_failure;

#[derive(Clone, Copy)]
pub(super) enum LevelPattern {
    AllOne,
    FixedListRepetition { dimension: usize },
}

pub(super) fn validate_level_stream(
    bytes: &[u8],
    expected_values: usize,
    pattern: LevelPattern,
) -> Result<(), EmbeddingColumnarError> {
    let mut cursor = 0_usize;
    let mut produced = 0_usize;
    while cursor < bytes.len() {
        if produced == expected_values {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let (header, header_bytes) = read_level_varint(
            bytes
                .get(cursor..)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?,
        )?;
        cursor = cursor
            .checked_add(header_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if header == 0 {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        if header & 1 == 0 {
            let run = header >> 1;
            if run == 0
                || run
                    > expected_values
                        .checked_sub(produced)
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            let value = *bytes
                .get(cursor)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            cursor = cursor
                .checked_add(1)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if value > 1 {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            for index in produced..produced + run {
                if value != expected_level(pattern, index)? {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            produced = produced
                .checked_add(run)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        } else {
            let groups = header >> 1;
            let run = groups
                .checked_mul(8)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let remaining = expected_values
                .checked_sub(produced)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if groups == 0 || run > remaining.saturating_add(7) {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            let group_bytes = groups;
            let packed = bytes
                .get(
                    cursor
                        ..cursor
                            .checked_add(group_bytes)
                            .ok_or(EmbeddingColumnarError::SizeOverflow)?,
                )
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            for index in 0..run {
                let value = (packed[index / 8] >> (index % 8)) & 1;
                if index < remaining {
                    if value != expected_level(pattern, produced + index)? {
                        return Err(parquet_failure(ParquetFailure::InvalidPage));
                    }
                } else if value != 0 {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            cursor = cursor
                .checked_add(group_bytes)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            produced = produced
                .checked_add(run.min(remaining))
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
    }
    if produced != expected_values {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

fn expected_level(pattern: LevelPattern, index: usize) -> Result<u8, EmbeddingColumnarError> {
    match pattern {
        LevelPattern::AllOne => Ok(1),
        LevelPattern::FixedListRepetition { dimension } => {
            if dimension == 0 {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            Ok(u8::from(!index.is_multiple_of(dimension)))
        }
    }
}

pub(in crate::columnar::parquet) fn read_level_varint(
    bytes: &[u8],
) -> Result<(usize, usize), EmbeddingColumnarError> {
    let mut value = 0_u32;
    for index in 0..5_usize {
        let byte = *bytes
            .get(index)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let payload = u32::from(byte & 0x7f);
        if index == 4 && payload > 0x0f {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        value |= payload
            .checked_shl(
                u32::try_from(index * 7).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            )
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        if byte & 0x80 == 0 {
            if index != 0 && payload == 0 {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            return Ok((
                usize::try_from(value).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
                index + 1,
            ));
        }
    }
    Err(parquet_failure(ParquetFailure::InvalidPage))
}

pub(super) fn validate_plain_byte_arrays(
    bytes: &[u8],
    expected_values: usize,
) -> Result<usize, EmbeddingColumnarError> {
    let mut cursor = 0_usize;
    let mut payload_bytes = 0_usize;
    for _ in 0..expected_values {
        let length_end = cursor
            .checked_add(size_of::<u32>())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let length = u32::from_le_bytes(
            bytes
                .get(cursor..length_end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?,
        );
        payload_bytes = payload_bytes
            .checked_add(usize::try_from(length).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        cursor = length_end
            .checked_add(usize::try_from(length).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if cursor > bytes.len() {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
    }
    if cursor != bytes.len() {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(payload_bytes)
}

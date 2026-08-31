use super::*;

pub(super) fn validate_plain_cell_ids(
    bytes: &[u8],
    row_link: &CellEmbeddingRowLink,
    start_row: usize,
    rows: usize,
) -> Result<(), EmbeddingColumnarError> {
    let entries = row_link
        .entries()
        .get(
            start_row
                ..start_row
                    .checked_add(rows)
                    .ok_or(EmbeddingColumnarError::SizeOverflow)?,
        )
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
    let mut cursor = 0_usize;
    for entry in entries {
        let length_end = cursor
            .checked_add(size_of::<u32>())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let length = usize::try_from(u32::from_le_bytes(
            bytes
                .get(cursor..length_end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?,
        ))
        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
        let value_end = length_end
            .checked_add(length)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let value = bytes
            .get(length_end..value_end)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        if value != entry.cell_id().as_str().as_bytes() {
            return Err(parquet_failure(ParquetFailure::InvalidCellOrder));
        }
        cursor = value_end;
    }
    if cursor != bytes.len() {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

pub(super) fn absolute_slice(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
) -> Option<&[u8]> {
    bytes.get(start.checked_sub(base_offset)?..end.checked_sub(base_offset)?)
}

pub(super) fn validate_optional_levels(
    bytes: &[u8],
    expected: &[bool],
) -> Result<(), EmbeddingColumnarError> {
    let mut cursor = 0_usize;
    let mut produced = 0_usize;
    while cursor < bytes.len() {
        if produced == expected.len() {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let (header, header_bytes) = read_level_varint(&bytes[cursor..])?;
        cursor = cursor
            .checked_add(header_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if header == 0 {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        if header & 1 == 0 {
            let run = header >> 1;
            if run == 0
                || produced
                    .checked_add(run)
                    .is_none_or(|value| value > expected.len())
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            let value = *bytes
                .get(cursor)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            cursor += 1;
            if value > 1
                || expected[produced..produced + run]
                    .iter()
                    .any(|present| u8::from(*present) != value)
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            produced += run;
        } else {
            let groups = header >> 1;
            let run = groups
                .checked_mul(8)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let remaining = expected
                .len()
                .checked_sub(produced)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if groups == 0 || run > remaining.saturating_add(7) {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            let packed = bytes
                .get(cursor..cursor + groups)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            for index in 0..run {
                let value = (packed[index / 8] >> (index % 8)) & 1;
                if index < remaining {
                    if value != u8::from(expected[produced + index]) {
                        return Err(parquet_failure(ParquetFailure::InvalidPage));
                    }
                } else if value != 0 {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            cursor += groups;
            produced += run.min(remaining);
        }
    }
    if produced != expected.len() {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

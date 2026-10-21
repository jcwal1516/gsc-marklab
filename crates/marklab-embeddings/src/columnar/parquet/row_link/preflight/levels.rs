use super::*;

pub(super) fn validate_plain_cell_ids(
    bytes: &[u8],
    row_link: &CellEmbeddingRowLink,
    start_row: usize,
    rows: usize,
) -> Result<(), ColumnarError> {
    let entries = row_link
        .entries()
        .get(
            start_row
                ..start_row
                    .checked_add(rows)
                    .ok_or(ColumnarError::SizeOverflow)?,
        )
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
    let mut cursor = 0_usize;
    for entry in entries {
        let length_end = cursor
            .checked_add(size_of::<u32>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let length = usize::try_from(u32::from_le_bytes(
            bytes
                .get(cursor..length_end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?,
        ))
        .map_err(|_| ColumnarError::SizeOverflow)?;
        let value_end = length_end
            .checked_add(length)
            .ok_or(ColumnarError::SizeOverflow)?;
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

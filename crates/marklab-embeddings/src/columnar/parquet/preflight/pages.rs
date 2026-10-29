use crate::columnar::{ColumnarError, ParquetFailure};

use super::super::{physical::read_data_page, profile::ROW_GROUP_ROWS};
use super::{
    levels::{validate_level_stream, validate_plain_byte_arrays, LevelPattern},
    parquet_failure,
};

#[derive(Clone, Copy)]
pub(super) struct ValidatedPages {
    pub(super) page_count: usize,
    pub(super) string_payload_bytes: usize,
}

pub(super) fn validate_pages(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
    column_index: usize,
    rows: usize,
    dimension: u32,
) -> Result<ValidatedPages, ColumnarError> {
    let mut cursor = start;
    let mut page_count = 0_usize;
    let mut aggregate_rows = 0_usize;
    let mut aggregate_values = 0_usize;
    let mut string_payload_bytes = 0_usize;
    while cursor < end {
        let physical = read_data_page(
            bytes,
            base_offset,
            cursor,
            end,
            ParquetFailure::InvalidPageHeader,
        )?;
        let page = physical
            .header
            .data_page_header_v2
            .as_ref()
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let page_rows = usize::try_from(page.num_rows)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let page_values = usize::try_from(page.num_values)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let page_nulls = usize::try_from(page.num_nulls)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let definition_bytes = usize::try_from(page.definition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let repetition_bytes = usize::try_from(page.repetition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let level_bytes = definition_bytes
            .checked_add(repetition_bytes)
            .ok_or(ColumnarError::SizeOverflow)?;
        let expected_page_values = if column_index == 1 {
            page_rows
                .checked_mul(usize::try_from(dimension).map_err(|_| ColumnarError::SizeOverflow)?)
                .ok_or(ColumnarError::SizeOverflow)?
        } else {
            page_rows
        };
        if page_rows == 0
            || page_rows > ROW_GROUP_ROWS
            || page_values != expected_page_values
            || page_nulls != 0
            || level_bytes > physical.body.len()
            || (column_index == 1 && (definition_bytes == 0 || repetition_bytes == 0))
            || (column_index != 1 && level_bytes != 0)
        {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let body = physical.body;
        let data_start = level_bytes;
        if column_index == 1 {
            let repetition = body
                .get(..repetition_bytes)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            let definition = body
                .get(repetition_bytes..level_bytes)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            validate_level_stream(
                repetition,
                page_values,
                LevelPattern::FixedListRepetition {
                    dimension: usize::try_from(dimension)
                        .map_err(|_| ColumnarError::SizeOverflow)?,
                },
            )?;
            validate_level_stream(definition, page_values, LevelPattern::AllOne)?;
            let values = body
                .get(data_start..)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            if values.len()
                != page_values
                    .checked_mul(size_of::<f32>())
                    .ok_or(ColumnarError::SizeOverflow)?
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
        } else {
            let values = body
                .get(data_start..)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            string_payload_bytes = string_payload_bytes
                .checked_add(validate_plain_byte_arrays(values, page_values)?)
                .ok_or(ColumnarError::SizeOverflow)?;
        }
        aggregate_rows = aggregate_rows
            .checked_add(page_rows)
            .ok_or(ColumnarError::SizeOverflow)?;
        aggregate_values = aggregate_values
            .checked_add(page_values)
            .ok_or(ColumnarError::SizeOverflow)?;
        page_count = page_count
            .checked_add(1)
            .ok_or(ColumnarError::SizeOverflow)?;
        cursor = physical.end;
    }
    let expected_values = if column_index == 1 {
        rows.checked_mul(usize::try_from(dimension).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)?
    } else {
        rows
    };
    if cursor != end || aggregate_rows != rows || aggregate_values != expected_values {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(ValidatedPages {
        page_count,
        string_payload_bytes,
    })
}

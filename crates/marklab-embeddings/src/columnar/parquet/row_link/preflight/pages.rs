use super::{levels::validate_plain_cell_ids, *};
use crate::columnar::parquet::physical::{read_data_page, validate_strings};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_pages(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
    column_index: usize,
    group_start_row: usize,
    group_rows: usize,
    row_link: &CellEmbeddingRowLink,
) -> Result<usize, ColumnarError> {
    validate_pages_profile(
        bytes,
        base_offset,
        start,
        end,
        column_index,
        group_start_row,
        group_rows,
        Some(row_link),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::columnar::parquet) fn validate_pages_profile(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
    column_index: usize,
    group_start_row: usize,
    group_rows: usize,
    row_link: Option<&CellEmbeddingRowLink>,
    statuses: Option<&[crate::EmbeddingStatus]>,
) -> Result<usize, ColumnarError> {
    let mut cursor = start;
    let mut page_count = 0_usize;
    let mut page_start_row = 0_usize;
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
        let rows = usize::try_from(page.num_rows)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let values = usize::try_from(page.num_values)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let nulls = usize::try_from(page.num_nulls)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let definitions = usize::try_from(page.definition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let repetitions = usize::try_from(page.repetition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        if rows == 0
            || values != rows
            || page_start_row
                .checked_add(rows)
                .is_none_or(|value| value > group_rows)
            || repetitions != 0
            || (column_index < 2 && (definitions != 0 || nulls != 0))
            || (column_index == 2 && definitions == 0)
        {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let body = physical.body;
        let data = body
            .get(definitions..)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        match column_index {
            0 => {
                let absolute_start = group_start_row
                    .checked_add(page_start_row)
                    .ok_or(ColumnarError::SizeOverflow)?;
                if let Some(row_link) = row_link {
                    validate_plain_cell_ids(data, row_link, absolute_start, rows)?;
                } else {
                    validate_strings(data, rows)?;
                }
            }
            1 => {
                if data.len()
                    != values
                        .checked_mul(size_of::<u64>())
                        .ok_or(ColumnarError::SizeOverflow)?
                {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            2 => {
                let absolute_start = group_start_row
                    .checked_add(page_start_row)
                    .ok_or(ColumnarError::SizeOverflow)?;
                let expected_present = |index: usize| -> Option<u8> {
                    let absolute = absolute_start.checked_add(index)?;
                    if let Some(link) = row_link {
                        Some(u8::from(
                            link.entries()
                                .get(absolute)?
                                .source_embedding_row()
                                .is_some(),
                        ))
                    } else {
                        let status = statuses?.get(absolute)?;
                        Some(u8::from(matches!(
                            status,
                            crate::EmbeddingStatus::Present | crate::EmbeddingStatus::QcRejected
                        )))
                    }
                };
                let expected_nulls = (0..rows).try_fold(0_usize, |count, index| {
                    expected_present(index)
                        .map(|present| count + usize::from(present == 0))
                        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))
                })?;
                if nulls != expected_nulls {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
                crate::columnar::parquet::levels::check_level_stream(
                    body.get(..definitions)
                        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?,
                    rows,
                    expected_present,
                )
                .map_err(crate::columnar::parquet::preflight::level_error)?;
                if data.len()
                    != values
                        .checked_sub(nulls)
                        .and_then(|value| value.checked_mul(size_of::<u64>()))
                        .ok_or(ColumnarError::SizeOverflow)?
                {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            _ => return Err(parquet_failure(ParquetFailure::InvalidPage)),
        }
        page_start_row = page_start_row
            .checked_add(rows)
            .ok_or(ColumnarError::SizeOverflow)?;
        page_count = page_count
            .checked_add(1)
            .ok_or(ColumnarError::SizeOverflow)?;
        cursor = physical.end;
    }
    if cursor != end || page_start_row != group_rows {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(page_count)
}

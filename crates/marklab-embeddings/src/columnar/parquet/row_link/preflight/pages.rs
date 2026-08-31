use super::{
    levels::{absolute_slice, validate_optional_levels, validate_plain_cell_ids},
    *,
};

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
) -> Result<usize, EmbeddingColumnarError> {
    let mut cursor = start;
    let mut page_count = 0_usize;
    let mut page_start_row = 0_usize;
    while cursor < end {
        let header_limit = cursor
            .checked_add(MAXIMUM_PAGE_HEADER_BYTES)
            .map(|value| value.min(end))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let window = absolute_slice(bytes, base_offset, cursor, header_limit)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        let mut protocol = BoundedCompactProtocol::new(window, page_limits());
        let header = PageHeader::read_from_in_protocol(&mut protocol)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        let header_bytes = protocol
            .consumed_bytes()
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        if header_bytes == 0
            || header_bytes > MAXIMUM_PAGE_HEADER_BYTES
            || !is_canonical_compact(
                &header,
                window
                    .get(..header_bytes)
                    .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPageHeader))?,
            )
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?
        {
            return Err(parquet_failure(ParquetFailure::InvalidPageHeader));
        }
        let body_bytes = usize::try_from(header.compressed_page_size)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let uncompressed_bytes = usize::try_from(header.uncompressed_page_size)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let body_start = cursor
            .checked_add(header_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let page_end = body_start
            .checked_add(body_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let page = header
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
        if body_bytes > MAXIMUM_PAGE_BYTES
            || body_bytes != uncompressed_bytes
            || page_end > end
            || header.type_ != PageType::DATA_PAGE_V2
            || header.crc.is_some()
            || header.data_page_header.is_some()
            || header.index_page_header.is_some()
            || header.dictionary_page_header.is_some()
            || page.encoding != Encoding::PLAIN
            || page.is_compressed != Some(false)
            || page.statistics.is_some()
            || rows == 0
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
        let body = absolute_slice(bytes, base_offset, body_start, page_end)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let data = body
            .get(definitions..)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        match column_index {
            0 => {
                let absolute_start = group_start_row
                    .checked_add(page_start_row)
                    .ok_or(EmbeddingColumnarError::SizeOverflow)?;
                validate_plain_cell_ids(data, row_link, absolute_start, rows)?;
            }
            1 => {
                if data.len()
                    != values
                        .checked_mul(size_of::<u64>())
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?
                {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            2 => {
                let absolute_start = group_start_row
                    .checked_add(page_start_row)
                    .ok_or(EmbeddingColumnarError::SizeOverflow)?;
                let entries = row_link
                    .entries()
                    .get(absolute_start..absolute_start + rows)
                    .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
                let expected_present = entries
                    .iter()
                    .map(|entry| entry.source_embedding_row().is_some())
                    .collect::<Vec<_>>();
                let expected_nulls = expected_present.iter().filter(|present| !**present).count();
                if nulls != expected_nulls {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
                validate_optional_levels(
                    body.get(..definitions)
                        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?,
                    &expected_present,
                )?;
                if data.len()
                    != values
                        .checked_sub(nulls)
                        .and_then(|value| value.checked_mul(size_of::<u64>()))
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?
                {
                    return Err(parquet_failure(ParquetFailure::InvalidPage));
                }
            }
            _ => return Err(parquet_failure(ParquetFailure::InvalidPage)),
        }
        page_start_row = page_start_row
            .checked_add(rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        page_count = page_count
            .checked_add(1)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        cursor = page_end;
    }
    if cursor != end || page_start_row != group_rows {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(page_count)
}

pub(super) fn page_limits() -> CompactLimits {
    CompactLimits {
        maximum_depth: 8,
        maximum_fields: 64,
        maximum_collection_elements: 16,
        maximum_total_elements: 64,
        maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
        maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
    }
}

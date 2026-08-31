use parquet::{
    format::{Encoding, PageHeader, PageType},
    thrift::TSerializable,
};

use crate::columnar::{EmbeddingColumnarError, ParquetFailure};

use super::super::{
    compact::{is_canonical_compact, BoundedCompactProtocol, CompactLimits},
    profile::{MAXIMUM_PAGE_BYTES, MAXIMUM_PAGE_HEADER_BYTES, ROW_GROUP_ROWS},
};
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
) -> Result<ValidatedPages, EmbeddingColumnarError> {
    let mut cursor = start;
    let mut page_count = 0_usize;
    let mut aggregate_rows = 0_usize;
    let mut aggregate_values = 0_usize;
    let mut string_payload_bytes = 0_usize;
    while cursor < end {
        let header_limit = cursor
            .checked_add(MAXIMUM_PAGE_HEADER_BYTES)
            .map(|value| value.min(end))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let header_window = absolute_slice(bytes, base_offset, cursor, header_limit)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        let limits = CompactLimits {
            maximum_depth: 8,
            maximum_fields: 64,
            maximum_collection_elements: 16,
            maximum_total_elements: 64,
            maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
            maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
        };
        let mut protocol = BoundedCompactProtocol::new(header_window, limits);
        let header = PageHeader::read_from_in_protocol(&mut protocol)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        let header_bytes = protocol
            .consumed_bytes()
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        if header_bytes == 0 || header_bytes > MAXIMUM_PAGE_HEADER_BYTES {
            return Err(parquet_failure(ParquetFailure::InvalidPageHeader));
        }
        let header_bytes_slice = header_window
            .get(..header_bytes)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPageHeader))?;
        if !is_canonical_compact(&header, header_bytes_slice)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPageHeader))?
        {
            return Err(parquet_failure(ParquetFailure::InvalidPageHeader));
        }
        let body_bytes = usize::try_from(header.compressed_page_size)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let uncompressed_bytes = usize::try_from(header.uncompressed_page_size)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        if body_bytes > MAXIMUM_PAGE_BYTES || body_bytes != uncompressed_bytes {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let body_start = cursor
            .checked_add(header_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let page_end = body_start
            .checked_add(body_bytes)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if page_end > end {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let page = header
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
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let expected_page_values = if column_index == 1 {
            page_rows
                .checked_mul(
                    usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
                )
                .ok_or(EmbeddingColumnarError::SizeOverflow)?
        } else {
            page_rows
        };
        if header.type_ != PageType::DATA_PAGE_V2
            || header.crc.is_some()
            || header.data_page_header.is_some()
            || header.index_page_header.is_some()
            || header.dictionary_page_header.is_some()
            || page.encoding != Encoding::PLAIN
            || page.is_compressed != Some(false)
            || page.statistics.is_some()
            || page_rows == 0
            || page_rows > ROW_GROUP_ROWS
            || page_values != expected_page_values
            || page_nulls != 0
            || level_bytes > body_bytes
            || (column_index == 1 && (definition_bytes == 0 || repetition_bytes == 0))
            || (column_index != 1 && level_bytes != 0)
        {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let body = absolute_slice(bytes, base_offset, body_start, page_end)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
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
                        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
                },
            )?;
            validate_level_stream(definition, page_values, LevelPattern::AllOne)?;
            let values = body
                .get(data_start..)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            if values.len()
                != page_values
                    .checked_mul(size_of::<f32>())
                    .ok_or(EmbeddingColumnarError::SizeOverflow)?
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
        } else {
            let values = body
                .get(data_start..)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            string_payload_bytes = string_payload_bytes
                .checked_add(validate_plain_byte_arrays(values, page_values)?)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
        aggregate_rows = aggregate_rows
            .checked_add(page_rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        aggregate_values = aggregate_values
            .checked_add(page_values)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        page_count = page_count
            .checked_add(1)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        cursor = page_end;
    }
    let expected_values = if column_index == 1 {
        rows.checked_mul(
            usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        )
        .ok_or(EmbeddingColumnarError::SizeOverflow)?
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

fn absolute_slice(bytes: &[u8], base_offset: usize, start: usize, end: usize) -> Option<&[u8]> {
    let local_start = start.checked_sub(base_offset)?;
    let local_end = end.checked_sub(base_offset)?;
    bytes.get(local_start..local_end)
}

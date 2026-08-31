use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;

use parquet::format::{
    ColumnChunk, ColumnMetaData, ColumnOrder, CompressionCodec, Encoding, FileMetaData, PageType,
};

use crate::columnar::{
    multiscale::{enforce_row_group_budget, SpatialParquetFailure},
    parquet::{
        compact::CompactLimits,
        profile::{
            CREATED_BY, MAXIMUM_FOOTER_BYTES, MAXIMUM_PAGE_HEADER_BYTES, PARQUET_MAGIC,
            ROW_GROUP_ROWS,
        },
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError,
};

pub(super) fn validate_footer_length(
    footer_len: usize,
    footer_length_offset: usize,
) -> Result<usize, MultiscaleColumnarError> {
    if footer_len == 0 || footer_len > MAXIMUM_FOOTER_BYTES {
        return Err(parquet_failure(SpatialParquetFailure::InvalidFooterLength));
    }
    let footer_start = footer_length_offset
        .checked_sub(footer_len)
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidFooterLength))?;
    if footer_start < PARQUET_MAGIC.len() {
        return Err(parquet_failure(SpatialParquetFailure::InvalidFooterLength));
    }
    Ok(footer_start)
}

pub(super) fn validate_column_features(
    column: &ColumnChunk,
    metadata: &ColumnMetaData,
) -> Result<(), MultiscaleColumnarError> {
    if column.file_path.is_some()
        || column.file_offset != 0
        || column.offset_index_offset.is_some()
        || column.offset_index_length.is_some()
        || column.column_index_offset.is_some()
        || column.column_index_length.is_some()
        || column.crypto_metadata.is_some()
        || column.encrypted_column_metadata.is_some()
        || metadata.codec != CompressionCodec::UNCOMPRESSED
        || metadata.encodings != [Encoding::PLAIN, Encoding::RLE]
        || metadata.key_value_metadata.is_some()
        || metadata.index_page_offset.is_some()
        || metadata.dictionary_page_offset.is_some()
        || metadata.statistics.is_some()
        || metadata.bloom_filter_offset.is_some()
        || metadata.bloom_filter_length.is_some()
        || metadata.size_statistics.is_some()
        || metadata.geospatial_statistics.is_some()
    {
        return Err(parquet_failure(SpatialParquetFailure::ForbiddenFeature));
    }
    Ok(())
}

/// Validates the physical row-group tree shared by every canonical C-05
/// multiscale Parquet profile. Schema, metadata, column identity, and page
/// contents remain with their format-specific callers.
pub(super) fn validate_row_group_tree<const MAX_COLUMNS: usize, C, P>(
    footer_start: usize,
    metadata: &FileMetaData,
    expected_rows: usize,
    column_count: usize,
    budgets: EmbeddingColumnarBudgets,
    mut validate_column: C,
    mut validate_pages: P,
) -> Result<(), MultiscaleColumnarError>
where
    C: FnMut(usize, usize, &ColumnMetaData) -> Result<(), MultiscaleColumnarError>,
    P: FnMut(usize, usize, usize, usize, usize) -> Result<usize, MultiscaleColumnarError>,
{
    if column_count > MAX_COLUMNS
        || metadata.created_by.as_deref() != Some(CREATED_BY)
        || metadata.encryption_algorithm.is_some()
        || metadata.footer_signing_key_metadata.is_some()
        || metadata.column_orders.as_ref().is_none_or(|orders| {
            orders.len() != column_count
                || orders
                    .iter()
                    .any(|order| !matches!(order, ColumnOrder::TYPEORDER(_)))
        })
    {
        return Err(parquet_failure(SpatialParquetFailure::InvalidMetadata));
    }
    let mut aggregate_rows = 0_usize;
    let mut next_offset = PARQUET_MAGIC.len();
    for (group_index, group) in metadata.row_groups.iter().enumerate() {
        let rows = expected_rows
            .checked_sub(aggregate_rows)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?
            .min(ROW_GROUP_ROWS);
        if usize::try_from(group.num_rows).ok() != Some(rows)
            || group.columns.len() != column_count
            || group.sorting_columns.is_some()
            || group.ordinal != i16::try_from(group_index).ok()
            || group
                .file_offset
                .and_then(|value| usize::try_from(value).ok())
                != Some(next_offset)
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidRowGroup));
        }
        let mut group_bytes = 0_usize;
        let mut column_ranges = [(0_usize, 0_usize); MAX_COLUMNS];
        for (column_index, column) in group.columns.iter().enumerate() {
            let column_metadata = column
                .meta_data
                .as_ref()
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowGroup))?;
            validate_column_features(column, column_metadata)?;
            validate_column(column_index, rows, column_metadata)?;
            if column_metadata.total_compressed_size <= 0
                || column_metadata.total_compressed_size != column_metadata.total_uncompressed_size
                || column_metadata
                    .encoding_stats
                    .as_ref()
                    .is_none_or(|statistics| {
                        statistics.len() != 1
                            || statistics[0].page_type != PageType::DATA_PAGE_V2
                            || statistics[0].encoding != Encoding::PLAIN
                            || statistics[0].count <= 0
                    })
            {
                return Err(parquet_failure(SpatialParquetFailure::InvalidRowGroup));
            }
            let start = usize::try_from(column_metadata.data_page_offset)
                .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidRowGroup))?;
            let length = usize::try_from(column_metadata.total_compressed_size)
                .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidRowGroup))?;
            if start != next_offset {
                return Err(parquet_failure(SpatialParquetFailure::InvalidRowGroup));
            }
            next_offset = start
                .checked_add(length)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            if next_offset > footer_start {
                return Err(parquet_failure(SpatialParquetFailure::InvalidRowGroup));
            }
            column_ranges[column_index] = (start, next_offset);
            group_bytes = group_bytes
                .checked_add(length)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        }
        if usize::try_from(group.total_byte_size).ok() != Some(group_bytes)
            || group
                .total_compressed_size
                .and_then(|value| usize::try_from(value).ok())
                != Some(group_bytes)
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidRowGroup));
        }
        enforce_row_group_budget(group_bytes, budgets)?;
        for (column_index, (start, end)) in
            column_ranges.iter().copied().take(column_count).enumerate()
        {
            let pages = validate_pages(start, end, column_index, aggregate_rows, rows)?;
            let column_metadata = group.columns[column_index]
                .meta_data
                .as_ref()
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowGroup))?;
            if column_metadata
                .encoding_stats
                .as_ref()
                .is_none_or(|statistics| i32::try_from(pages).ok() != Some(statistics[0].count))
            {
                return Err(parquet_failure(SpatialParquetFailure::InvalidPage));
            }
        }
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    if aggregate_rows != expected_rows || next_offset != footer_start {
        return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
    }
    Ok(())
}

pub(super) fn absolute_slice(bytes: &[u8], base: usize, start: usize, end: usize) -> Option<&[u8]> {
    bytes.get(start.checked_sub(base)?..end.checked_sub(base)?)
}

pub(super) fn validate_plain_strings<'a>(
    bytes: &[u8],
    expected: impl Iterator<Item = &'a str>,
) -> Result<(), MultiscaleColumnarError> {
    let mut cursor = 0_usize;
    for value in expected {
        let length_end = cursor
            .checked_add(size_of::<u32>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let length = usize::try_from(u32::from_le_bytes(
            bytes
                .get(cursor..length_end)
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?,
        ))
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
        let end = length_end
            .checked_add(length)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        if bytes.get(length_end..end) != Some(value.as_bytes()) {
            return Err(parquet_failure(SpatialParquetFailure::InvalidCanonicalRows));
        }
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(parquet_failure(SpatialParquetFailure::InvalidPage));
    }
    Ok(())
}

pub(super) fn validate_plain_u64(
    bytes: &[u8],
    expected: impl Iterator<Item = u64>,
) -> Result<(), MultiscaleColumnarError> {
    let mut count = 0_usize;
    for (index, value) in expected.enumerate() {
        let start = index
            .checked_mul(size_of::<u64>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let end = start
            .checked_add(size_of::<u64>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let observed = u64::from_le_bytes(
            bytes
                .get(start..end)
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?,
        );
        if observed != value {
            return Err(parquet_failure(SpatialParquetFailure::InvalidCanonicalRows));
        }
        count = index
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    if bytes.len()
        != count
            .checked_mul(size_of::<u64>())
            .ok_or(MultiscaleColumnarError::SizeOverflow)?
    {
        return Err(parquet_failure(SpatialParquetFailure::InvalidPage));
    }
    Ok(())
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

pub(super) fn read_exact_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    offset: u64,
    buffer: &mut [u8],
) -> Result<(), MultiscaleColumnarError> {
    reader
        .seek(SeekFrom::Start(offset))
        .and_then(|_| reader.read_exact(buffer))
        .map_err(|_| parquet_failure(SpatialParquetFailure::ArtifactRead))
}

pub(super) fn parquet_failure(reason: SpatialParquetFailure) -> MultiscaleColumnarError {
    MultiscaleColumnarError::Parquet { reason }
}

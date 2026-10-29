use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;

use crate::columnar::parquet::levels::LevelStreamError;

use crate::columnar::parquet::format::{
    ColumnChunk, ColumnMetaData, CompressionCodec, Encoding, FileMetaData, PageHeader, PageType,
    TSerializable,
};
use crate::columnar::parquet::schema::has_writer_column_orders;
use parquet::file::metadata::{ParquetMetaDataReader, RowGroupMetaData};

use crate::columnar::{
    enforce_retained_budget, enforce_row_group_budget,
    parquet::{
        compact::{is_canonical_compact, BoundedCompactProtocol, CompactLimits},
        preflight::estimate_raw_preflight_bytes,
        profile::{
            CREATED_BY, MAXIMUM_FOOTER_BYTES, MAXIMUM_PAGE_BYTES, MAXIMUM_PAGE_HEADER_BYTES,
            PARQUET_MAGIC, ROW_GROUP_ROWS, TRAILER_BYTES,
        },
    },
    ColumnarError, EmbeddingColumnarBudgets, ParquetFailure,
};

/// Bounded physical footer admission shared by canonical multiscale profiles.
pub(super) struct RawFooter {
    pub(super) footer: Vec<u8>,
    pub(super) raw: FileMetaData,
    pub(super) footer_start: usize,
    pub(super) raw_retained: usize,
    pub(super) stock_retained: usize,
}

pub(super) fn read_footer<R: Read + Seek + ?Sized>(
    reader: &mut R,
    encoded_byte_len: u64,
    compact_limits: impl FnOnce() -> Result<CompactLimits, ColumnarError>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<RawFooter, ColumnarError> {
    let minimum = u64::try_from(PARQUET_MAGIC.len() + TRAILER_BYTES)
        .map_err(|_| ColumnarError::SizeOverflow)?;
    if encoded_byte_len < minimum {
        return Err(parquet_failure(ParquetFailure::InvalidMagic));
    }
    if reader
        .seek(SeekFrom::End(0))
        .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?
        != encoded_byte_len
    {
        return Err(parquet_failure(ParquetFailure::ArtifactRead));
    }
    let mut leading = [0_u8; 4];
    read_exact_at(reader, 0, &mut leading)?;
    let mut trailer = [0_u8; TRAILER_BYTES];
    read_exact_at(
        reader,
        encoded_byte_len - TRAILER_BYTES as u64,
        &mut trailer,
    )?;
    if &leading != PARQUET_MAGIC || trailer.get(4..) != Some(PARQUET_MAGIC) {
        return Err(parquet_failure(ParquetFailure::InvalidMagic));
    }
    let footer_len = usize::try_from(u32::from_le_bytes(
        trailer[..4]
            .try_into()
            .map_err(|_| parquet_failure(ParquetFailure::InvalidFooterLength))?,
    ))
    .map_err(|_| ColumnarError::SizeOverflow)?;
    let file_len = usize::try_from(encoded_byte_len).map_err(|_| ColumnarError::SizeOverflow)?;
    let footer_length_offset = file_len
        .checked_sub(TRAILER_BYTES)
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?;
    let footer_start = validate_footer_length(footer_len, footer_length_offset)?;
    let limits = compact_limits()?;
    let raw_retained = estimate_raw_preflight_bytes(footer_len, limits)
        .map_err(|_| ColumnarError::SizeOverflow)?;
    let stock_retained = estimate_raw_preflight_bytes(MAXIMUM_FOOTER_BYTES, limits)
        .map_err(|_| ColumnarError::SizeOverflow)?
        .checked_mul(2)
        .ok_or(ColumnarError::SizeOverflow)?;
    enforce_retained_budget(raw_retained.max(stock_retained), budgets)?;
    let mut footer = Vec::new();
    footer
        .try_reserve_exact(footer_len)
        .map_err(|_| ColumnarError::AllocationFailed {
            requested: footer_len,
        })?;
    footer.resize(footer_len, 0);
    read_exact_at(
        reader,
        u64::try_from(footer_start).map_err(|_| ColumnarError::SizeOverflow)?,
        &mut footer,
    )?;
    let mut protocol = BoundedCompactProtocol::new(&footer, limits);
    let raw = FileMetaData::read_from_in_protocol(&mut protocol)
        .map_err(|_| parquet_failure(ParquetFailure::InvalidFooter))?;
    if protocol
        .consumed_bytes()
        .map_err(|_| parquet_failure(ParquetFailure::InvalidFooter))?
        != footer.len()
        || !is_canonical_compact(&raw, &footer)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidFooter))?
    {
        return Err(parquet_failure(ParquetFailure::InvalidFooter));
    }
    Ok(RawFooter {
        footer,
        raw,
        footer_start,
        raw_retained,
        stock_retained,
    })
}

pub(super) fn decode_stock_metadata(
    footer: &[u8],
    row_count: usize,
    stock_retained: usize,
) -> Result<parquet::file::metadata::ParquetMetaData, ColumnarError> {
    let stock = ParquetMetaDataReader::decode_metadata(footer)
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    if stock.memory_size() > stock_retained
        || stock.file_metadata().version() != 2
        || usize::try_from(stock.file_metadata().num_rows()).ok() != Some(row_count)
        || stock.num_row_groups() != row_count.div_ceil(ROW_GROUP_ROWS)
        || stock.file_metadata().created_by() != Some(CREATED_BY)
        || stock.page_index().is_some()
    {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    Ok(stock)
}

pub(in crate::columnar::parquet) fn validate_footer_length(
    footer_len: usize,
    footer_length_offset: usize,
) -> Result<usize, ColumnarError> {
    if footer_len == 0 || footer_len > MAXIMUM_FOOTER_BYTES {
        return Err(parquet_failure(ParquetFailure::InvalidFooterLength));
    }
    let footer_start = footer_length_offset
        .checked_sub(footer_len)
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidFooterLength))?;
    if footer_start < PARQUET_MAGIC.len() {
        return Err(parquet_failure(ParquetFailure::InvalidFooterLength));
    }
    Ok(footer_start)
}

pub(in crate::columnar::parquet) fn validate_column_features(
    column: &ColumnChunk,
    metadata: &ColumnMetaData,
) -> Result<(), ColumnarError> {
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
        return Err(parquet_failure(ParquetFailure::ForbiddenFeature));
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn validated_stock_group_range(
    group: &RowGroupMetaData,
    expected_columns: usize,
) -> Result<(u64, usize), ColumnarError> {
    if group.num_columns() != expected_columns {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    let start = u64::try_from(group.column(0).data_page_offset())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let mut next = start;
    for column in group.columns() {
        let offset = u64::try_from(column.data_page_offset())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let length = u64::try_from(column.compressed_size())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        if offset != next || length == 0 {
            return Err(parquet_failure(ParquetFailure::StockDecode));
        }
        next = next
            .checked_add(length)
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    Ok((
        start,
        usize::try_from(next - start).map_err(|_| ColumnarError::SizeOverflow)?,
    ))
}

/// Validates the physical row-group tree shared by every canonical
/// multiscale Parquet profile. Schema, metadata, column identity, and page
/// contents remain with their format-specific callers.
pub(in crate::columnar::parquet) fn validate_row_group_tree<const MAX_COLUMNS: usize, C, P>(
    footer_start: usize,
    metadata: &FileMetaData,
    expected_rows: usize,
    column_count: usize,
    budgets: EmbeddingColumnarBudgets,
    mut validate_column: C,
    mut validate_pages: P,
) -> Result<(), ColumnarError>
where
    C: FnMut(usize, usize, &ColumnMetaData) -> Result<(), ColumnarError>,
    P: FnMut(usize, usize, usize, usize, usize) -> Result<usize, ColumnarError>,
{
    if column_count > MAX_COLUMNS
        || metadata.created_by.as_deref() != Some(CREATED_BY)
        || metadata.encryption_algorithm.is_some()
        || metadata.footer_signing_key_metadata.is_some()
        || metadata.column_orders.as_ref().is_none_or(|orders| {
            orders.len() != column_count || !has_writer_column_orders(&metadata.schema, orders)
        })
    {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    let mut aggregate_rows = 0_usize;
    let mut next_offset = PARQUET_MAGIC.len();
    for (group_index, group) in metadata.row_groups.iter().enumerate() {
        let rows = expected_rows
            .checked_sub(aggregate_rows)
            .ok_or(ColumnarError::SizeOverflow)?
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
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        let mut group_bytes = 0_usize;
        let mut column_ranges = [(0_usize, 0_usize); MAX_COLUMNS];
        for (column_index, column) in group.columns.iter().enumerate() {
            let column_metadata = column
                .meta_data
                .as_ref()
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowGroup))?;
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
                return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
            }
            let start = usize::try_from(column_metadata.data_page_offset)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidRowGroup))?;
            let length = usize::try_from(column_metadata.total_compressed_size)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidRowGroup))?;
            if start != next_offset {
                return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
            }
            next_offset = start
                .checked_add(length)
                .ok_or(ColumnarError::SizeOverflow)?;
            if next_offset > footer_start {
                return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
            }
            column_ranges[column_index] = (start, next_offset);
            group_bytes = group_bytes
                .checked_add(length)
                .ok_or(ColumnarError::SizeOverflow)?;
        }
        if usize::try_from(group.total_byte_size).ok() != Some(group_bytes)
            || group
                .total_compressed_size
                .and_then(|value| usize::try_from(value).ok())
                != Some(group_bytes)
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        enforce_row_group_budget(group_bytes, budgets)?;
        for (column_index, (start, end)) in
            column_ranges.iter().copied().take(column_count).enumerate()
        {
            let pages = validate_pages(start, end, column_index, aggregate_rows, rows)?;
            let column_metadata = group.columns[column_index]
                .meta_data
                .as_ref()
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowGroup))?;
            if column_metadata
                .encoding_stats
                .as_ref()
                .is_none_or(|statistics| i32::try_from(pages).ok() != Some(statistics[0].count))
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
        }
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    if aggregate_rows != expected_rows || next_offset != footer_start {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn absolute_slice(
    bytes: &[u8],
    base: usize,
    start: usize,
    end: usize,
) -> Option<&[u8]> {
    bytes.get(start.checked_sub(base)?..end.checked_sub(base)?)
}

pub(in crate::columnar::parquet) struct PhysicalDataPage<'a> {
    pub header: PageHeader,
    pub body: &'a [u8],
    pub end: usize,
}

pub(in crate::columnar::parquet) fn page_header_length_is_valid(header_bytes: usize) -> bool {
    header_bytes != 0 && header_bytes <= MAXIMUM_PAGE_HEADER_BYTES
}

pub(in crate::columnar::parquet) fn read_data_page<'a>(
    bytes: &'a [u8],
    base_offset: usize,
    start: usize,
    column_end: usize,
    header_failure: ParquetFailure,
) -> Result<PhysicalDataPage<'a>, ColumnarError> {
    let header_limit = start
        .checked_add(MAXIMUM_PAGE_HEADER_BYTES)
        .map(|value| value.min(column_end))
        .ok_or(ColumnarError::SizeOverflow)?;
    let window = absolute_slice(bytes, base_offset, start, header_limit)
        .ok_or_else(|| parquet_failure(header_failure))?;
    let mut protocol = BoundedCompactProtocol::new(window, page_limits());
    let header = PageHeader::read_from_in_protocol(&mut protocol)
        .map_err(|_| parquet_failure(header_failure))?;
    let header_bytes = protocol
        .consumed_bytes()
        .map_err(|_| parquet_failure(header_failure))?;
    if !page_header_length_is_valid(header_bytes)
        || !is_canonical_compact(
            &header,
            window
                .get(..header_bytes)
                .ok_or_else(|| parquet_failure(header_failure))?,
        )
        .map_err(|_| parquet_failure(header_failure))?
    {
        return Err(parquet_failure(header_failure));
    }
    let body_bytes = usize::try_from(header.compressed_page_size)
        .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
    let uncompressed = usize::try_from(header.uncompressed_page_size)
        .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
    let body_start = start
        .checked_add(header_bytes)
        .ok_or(ColumnarError::SizeOverflow)?;
    let end = body_start
        .checked_add(body_bytes)
        .ok_or(ColumnarError::SizeOverflow)?;
    if body_bytes > MAXIMUM_PAGE_BYTES
        || body_bytes != uncompressed
        || end > column_end
        || header.type_ != PageType::DATA_PAGE_V2
        || header.crc.is_some()
        || header.data_page_header.is_some()
        || header.index_page_header.is_some()
        || header.dictionary_page_header.is_some()
        || header.data_page_header_v2.as_ref().is_none_or(|page| {
            page.encoding != Encoding::PLAIN
                || page.is_compressed != Some(false)
                || page.statistics.is_some()
        })
    {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    let body = absolute_slice(bytes, base_offset, body_start, end)
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
    Ok(PhysicalDataPage { header, body, end })
}

pub(in crate::columnar::parquet) fn validate_plain_strings<'a>(
    bytes: &[u8],
    expected: impl Iterator<Item = &'a str>,
) -> Result<(), ColumnarError> {
    let mut cursor = 0_usize;
    for value in expected {
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
        let end = length_end
            .checked_add(length)
            .ok_or(ColumnarError::SizeOverflow)?;
        if bytes.get(length_end..end) != Some(value.as_bytes()) {
            return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
        }
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn validate_plain_u64(
    bytes: &[u8],
    expected: impl Iterator<Item = u64>,
) -> Result<(), ColumnarError> {
    let mut count = 0_usize;
    for (index, value) in expected.enumerate() {
        let start = index
            .checked_mul(size_of::<u64>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let end = start
            .checked_add(size_of::<u64>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let observed = u64::from_le_bytes(
            bytes
                .get(start..end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?,
        );
        if observed != value {
            return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
        }
        count = index.checked_add(1).ok_or(ColumnarError::SizeOverflow)?;
    }
    if bytes.len()
        != count
            .checked_mul(size_of::<u64>())
            .ok_or(ColumnarError::SizeOverflow)?
    {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn validate_strings(
    bytes: &[u8],
    rows: usize,
) -> Result<(), ColumnarError> {
    let mut cursor = 0_usize;
    for _ in 0..rows {
        let prefix_end = cursor.checked_add(4).ok_or(ColumnarError::SizeOverflow)?;
        let length = u32::from_le_bytes(
            bytes
                .get(cursor..prefix_end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?,
        ) as usize;
        let end = prefix_end
            .checked_add(length)
            .ok_or(ColumnarError::SizeOverflow)?;
        std::str::from_utf8(
            bytes
                .get(prefix_end..end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?,
        )
        .map_err(|_| parquet_failure(ParquetFailure::InvalidCanonicalRows))?;
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn validate_fixed_width(
    bytes: &[u8],
    count: usize,
    width: usize,
) -> Result<(), ColumnarError> {
    if bytes.len()
        != count
            .checked_mul(width)
            .ok_or(ColumnarError::SizeOverflow)?
    {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn validate_f32(
    bytes: &[u8],
    count: usize,
) -> Result<(), ColumnarError> {
    validate_fixed_width(bytes, count, 4)?;
    for chunk in bytes.as_chunks::<4>().0 {
        let bits = u32::from_le_bytes(*chunk);
        let value = f32::from_bits(bits);
        if !value.is_finite() || (value == 0.0 && bits != 0) {
            return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
        }
    }
    Ok(())
}

/// The multiscale error for a rejected level stream.
pub(in crate::columnar::parquet) fn level_error(error: LevelStreamError) -> ColumnarError {
    match error {
        LevelStreamError::Invalid => parquet_failure(ParquetFailure::InvalidPage),
        LevelStreamError::Overflow => ColumnarError::SizeOverflow,
    }
}

pub(in crate::columnar::parquet) fn page_limits() -> CompactLimits {
    CompactLimits {
        maximum_depth: 8,
        maximum_fields: 64,
        maximum_collection_elements: 16,
        maximum_total_elements: 64,
        maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
        maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
    }
}

pub(in crate::columnar::parquet) fn read_exact_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    offset: u64,
    buffer: &mut [u8],
) -> Result<(), ColumnarError> {
    reader
        .seek(SeekFrom::Start(offset))
        .and_then(|_| reader.read_exact(buffer))
        .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))
}

pub(in crate::columnar::parquet) fn parquet_failure(reason: ParquetFailure) -> ColumnarError {
    ColumnarError::Parquet { reason }
}

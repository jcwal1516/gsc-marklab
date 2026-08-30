use std::io::{Cursor, Read, Seek, SeekFrom};
use std::mem::size_of;

use marklab_project::ContentDigest;
use parquet::{
    file::metadata::{ParquetMetaData, ParquetMetaDataReader},
    format::{
        ColumnOrder, ConvertedType, Encoding, FieldRepetitionType, FileMetaData, LogicalType,
        PageHeader, PageType, SchemaElement, Type,
    },
    thrift::TSerializable,
};

use crate::PatchRegionLink;

pub(super) use super::super::physical::parquet_failure;
use super::super::physical::{
    absolute_slice, page_limits, read_exact_at, validate_column_features, validate_footer_length,
};
use super::profile::{COLUMN_COUNT, ROOT};
use crate::columnar::{
    multiscale::{
        enforce_decoded_budget, enforce_file_budget, enforce_retained_budget,
        enforce_row_group_budget, patch_region_decoded_bytes, patch_region_metadata,
        validate_patch_region_domain, PATCH_REGION_METADATA_KEYS,
    },
    parquet::{
        compact::{is_canonical_compact, BoundedCompactProtocol, CompactLimits},
        preflight::estimate_raw_preflight_bytes,
        profile::{
            CREATED_BY, MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_FOOTER_BYTES,
            MAXIMUM_PAGE_BYTES, MAXIMUM_PAGE_HEADER_BYTES, PARQUET_MAGIC, ROW_GROUP_ROWS,
            TRAILER_BYTES,
        },
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialParquetFailure,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

const PAGE_ROWS: usize = 1_024;

/// Validated structural declaration for one canonical Parquet patch-region link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatchRegionParquetPreflight {
    row_count: u64,
    row_group_count: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
}

impl PatchRegionParquetPreflight {
    pub(super) fn from_prepared(prepared: &PreparedPatchRegionParquet) -> Self {
        Self {
            row_count: prepared.row_count,
            row_group_count: prepared.row_group_count,
            encoded_byte_len: prepared.encoded_byte_len,
            content_digest: prepared.content_digest,
            retained_preflight_bytes: prepared.retained_preflight_bytes,
        }
    }

    /// Exact canonical nonzero-relation row count.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Number of canonical row groups.
    pub fn row_group_count(self) -> u32 {
        self.row_group_count
    }

    /// Exact encoded Parquet file length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// SHA-256 of the exact preflighted bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }
}

pub(super) struct PreparedPatchRegionParquet {
    pub(super) row_count: u64,
    pub(super) row_group_count: u32,
    pub(super) encoded_byte_len: u64,
    pub(super) content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
    pub(super) metadata: ParquetMetaData,
}

/// Raw-preflight borrowed canonical Parquet patch-region-link bytes.
pub fn preflight_patch_region_link_parquet_bytes(
    bytes: &[u8],
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchRegionParquetPreflight, MultiscaleColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    let prepared = prepare_reader(
        &mut Cursor::new(bytes),
        encoded,
        ContentDigest::from_bytes(bytes),
        link,
        budgets,
    )?;
    Ok(PatchRegionParquetPreflight::from_prepared(&prepared))
}

pub(super) fn prepare_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PreparedPatchRegionParquet, MultiscaleColumnarError> {
    enforce_file_budget(encoded_byte_len, budgets)?;
    validate_patch_region_domain(link)?;
    let row_count = link.nonzero_relation_count();
    enforce_decoded_budget(decoded_bytes(link)?, budgets)?;
    let minimum = u64::try_from(PARQUET_MAGIC.len() + TRAILER_BYTES)
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    if encoded_byte_len < minimum {
        return Err(parquet_failure(SpatialParquetFailure::InvalidMagic));
    }
    if reader
        .seek(SeekFrom::End(0))
        .map_err(|_| parquet_failure(SpatialParquetFailure::ArtifactRead))?
        != encoded_byte_len
    {
        return Err(parquet_failure(SpatialParquetFailure::ArtifactRead));
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
        return Err(parquet_failure(SpatialParquetFailure::InvalidMagic));
    }
    let footer_len =
        usize::try_from(u32::from_le_bytes(trailer[..4].try_into().map_err(
            |_| parquet_failure(SpatialParquetFailure::InvalidFooterLength),
        )?))
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    let file_len =
        usize::try_from(encoded_byte_len).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    let footer_length_offset = file_len
        .checked_sub(TRAILER_BYTES)
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidFooterLength))?;
    let footer_start = validate_footer_length(footer_len, footer_length_offset)?;
    let expected_groups = row_count.div_ceil(ROW_GROUP_ROWS);
    let limits = footer_compact_limits(expected_groups)?;
    let raw_retained = estimate_raw_preflight_bytes(footer_len, limits)
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    let stock_retained = estimate_raw_preflight_bytes(MAXIMUM_FOOTER_BYTES, limits)
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?
        .checked_mul(2)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let mut retained = raw_retained.max(stock_retained);
    enforce_retained_budget(retained, budgets)?;
    let mut footer = Vec::new();
    footer.try_reserve_exact(footer_len).map_err(|_| {
        MultiscaleColumnarError::AllocationFailed {
            requested: footer_len,
        }
    })?;
    footer.resize(footer_len, 0);
    read_exact_at(
        reader,
        u64::try_from(footer_start).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        &mut footer,
    )?;
    let mut protocol = BoundedCompactProtocol::new(&footer, limits);
    let raw = FileMetaData::read_from_in_protocol(&mut protocol)
        .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidFooter))?;
    if protocol
        .consumed_bytes()
        .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidFooter))?
        != footer.len()
        || !is_canonical_compact(&raw, &footer)
            .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidFooter))?
    {
        return Err(parquet_failure(SpatialParquetFailure::InvalidFooter));
    }
    validate_metadata_tree(
        footer_start,
        &raw,
        link,
        budgets,
        |start, end, column, group_start, rows| {
            let chunk_len = end
                .checked_sub(start)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            let peak = raw_retained
                .checked_add(chunk_len)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            retained = retained.max(peak);
            enforce_retained_budget(peak, budgets)?;
            let mut chunk = Vec::new();
            chunk.try_reserve_exact(chunk_len).map_err(|_| {
                MultiscaleColumnarError::AllocationFailed {
                    requested: chunk_len,
                }
            })?;
            chunk.resize(chunk_len, 0);
            read_exact_at(
                reader,
                u64::try_from(start).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
                &mut chunk,
            )?;
            validate_pages(&chunk, start, start, end, column, group_start, rows, link)
        },
    )?;
    let row_group_count = raw.row_groups.len();
    drop(raw);
    let stock = ParquetMetaDataReader::decode_metadata(&footer)
        .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
    if stock.memory_size() > stock_retained
        || stock.file_metadata().version() != 2
        || usize::try_from(stock.file_metadata().num_rows()).ok() != Some(row_count)
        || stock.num_row_groups() != expected_groups
        || stock.file_metadata().created_by() != Some(CREATED_BY)
        || stock.column_index().is_some()
        || stock.offset_index().is_some()
    {
        return Err(parquet_failure(SpatialParquetFailure::StockDecode));
    }
    Ok(PreparedPatchRegionParquet {
        row_count: u64::try_from(row_count).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        row_group_count: u32::try_from(row_group_count)
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_preflight_bytes: retained,
        metadata: stock,
    })
}

fn validate_metadata_tree<F>(
    footer_start: usize,
    metadata: &FileMetaData,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
    mut page_validator: F,
) -> Result<(), MultiscaleColumnarError>
where
    F: FnMut(usize, usize, usize, usize, usize) -> Result<usize, MultiscaleColumnarError>,
{
    let expected_rows = link.nonzero_relation_count();
    if metadata.version != 2
        || usize::try_from(metadata.num_rows).ok() != Some(expected_rows)
        || metadata.row_groups.len() != expected_rows.div_ceil(ROW_GROUP_ROWS)
    {
        return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
    }
    validate_schema(&metadata.schema)?;
    validate_metadata(metadata, link)?;
    if metadata.created_by.as_deref() != Some(CREATED_BY)
        || metadata.encryption_algorithm.is_some()
        || metadata.footer_signing_key_metadata.is_some()
        || metadata.column_orders.as_ref().is_none_or(|orders| {
            orders.len() != COLUMN_COUNT
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
            || group.columns.len() != COLUMN_COUNT
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
        let mut column_ranges = [(0_usize, 0_usize); COLUMN_COUNT];
        for (column_index, column) in group.columns.iter().enumerate() {
            let column_metadata = column
                .meta_data
                .as_ref()
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowGroup))?;
            validate_column_features(column, column_metadata)?;
            let (expected_type, expected_path) = column_profile(column_index)?;
            if column_metadata.type_ != expected_type
                || column_metadata
                    .path_in_schema
                    .iter()
                    .map(String::as_str)
                    .ne(std::iter::once(expected_path))
                || usize::try_from(column_metadata.num_values).ok() != Some(rows)
                || column_metadata.total_compressed_size <= 0
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
        for (column_index, (start, end)) in column_ranges.iter().copied().enumerate() {
            let pages = page_validator(start, end, column_index, aggregate_rows, rows)?;
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

#[allow(clippy::too_many_arguments)]
fn validate_pages(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
    column_index: usize,
    group_start_row: usize,
    group_rows: usize,
    link: &PatchRegionLink,
) -> Result<usize, MultiscaleColumnarError> {
    let mut cursor = start;
    let mut page_count = 0_usize;
    let mut page_start_row = 0_usize;
    while cursor < end {
        let header_limit = cursor
            .checked_add(MAXIMUM_PAGE_HEADER_BYTES)
            .map(|value| value.min(end))
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let window = absolute_slice(bytes, base_offset, cursor, header_limit)
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?;
        let mut protocol = BoundedCompactProtocol::new(window, page_limits());
        let header = PageHeader::read_from_in_protocol(&mut protocol)
            .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?;
        let header_bytes = protocol
            .consumed_bytes()
            .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?;
        if header_bytes == 0
            || header_bytes > MAXIMUM_PAGE_HEADER_BYTES
            || !is_canonical_compact(
                &header,
                window
                    .get(..header_bytes)
                    .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?,
            )
            .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidPage));
        }
        let body_bytes = usize::try_from(header.compressed_page_size)
            .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?;
        let uncompressed = usize::try_from(header.uncompressed_page_size)
            .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?;
        let body_start = cursor
            .checked_add(header_bytes)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let page_end = body_start
            .checked_add(body_bytes)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let page = header
            .data_page_header_v2
            .as_ref()
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?;
        let rows = usize::try_from(page.num_rows)
            .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?;
        if body_bytes > MAXIMUM_PAGE_BYTES
            || body_bytes != uncompressed
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
            || rows != group_rows.saturating_sub(page_start_row).min(PAGE_ROWS)
            || usize::try_from(page.num_values).ok() != Some(rows)
            || page.num_nulls != 0
            || page.definition_levels_byte_length != 0
            || page.repetition_levels_byte_length != 0
            || page_start_row
                .checked_add(rows)
                .is_none_or(|value| value > group_rows)
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidPage));
        }
        let body = absolute_slice(bytes, base_offset, body_start, page_end)
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?;
        let absolute_start = group_start_row
            .checked_add(page_start_row)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        validate_plain_values(body, column_index, absolute_start, rows, link)?;
        page_start_row = page_start_row
            .checked_add(rows)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        page_count = page_count
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        cursor = page_end;
    }
    if cursor != end || page_start_row != group_rows {
        return Err(parquet_failure(SpatialParquetFailure::InvalidPage));
    }
    Ok(page_count)
}

fn validate_plain_values(
    bytes: &[u8],
    column: usize,
    start: usize,
    rows: usize,
    link: &PatchRegionLink,
) -> Result<(), MultiscaleColumnarError> {
    let end = start
        .checked_add(rows)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let selected = link
        .nonzero_relations()
        .get(start..end)
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?;
    match column {
        0 => validate_plain_strings(bytes, selected.iter().map(|row| row.patch_id().as_str())),
        1 => validate_plain_strings(bytes, selected.iter().map(|row| row.region_id().as_str())),
        2 => validate_plain_strings(bytes, selected.iter().map(|row| row.relation().wire_name())),
        3 => validate_plain_u64(bytes, selected.iter().map(|row| row.numerator())),
        4 => validate_plain_u64(bytes, selected.iter().map(|row| row.denominator())),
        _ => Err(parquet_failure(SpatialParquetFailure::InvalidPage)),
    }
}

fn validate_plain_strings<'a>(
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

fn validate_plain_u64(
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

fn validate_schema(schema: &[SchemaElement]) -> Result<(), MultiscaleColumnarError> {
    if schema.len() != COLUMN_COUNT + 1
        || !is_group(&schema[0], ROOT, COLUMN_COUNT as i32)
        || !is_utf8(&schema[1], "patch_id")
        || !is_utf8(&schema[2], "region_id")
        || !is_utf8(&schema[3], "relation")
        || !is_u64(&schema[4], "overlap_numerator")
        || !is_u64(&schema[5], "overlap_denominator")
    {
        return Err(parquet_failure(SpatialParquetFailure::InvalidSchema));
    }
    Ok(())
}

fn is_group(element: &SchemaElement, name: &str, children: i32) -> bool {
    element.type_.is_none()
        && element.type_length.is_none()
        && element.repetition_type.is_none()
        && element.name == name
        && element.num_children == Some(children)
        && element.converted_type.is_none()
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && element.logical_type.is_none()
}

fn is_utf8(element: &SchemaElement, name: &str) -> bool {
    element.type_ == Some(Type::BYTE_ARRAY)
        && element.type_length.is_none()
        && element.repetition_type == Some(FieldRepetitionType::REQUIRED)
        && element.name == name
        && element.num_children.is_none()
        && element.converted_type == Some(ConvertedType::UTF8)
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && matches!(element.logical_type, Some(LogicalType::STRING(_)))
}

fn is_u64(element: &SchemaElement, name: &str) -> bool {
    element.type_ == Some(Type::INT64)
        && element.type_length.is_none()
        && element.repetition_type == Some(FieldRepetitionType::REQUIRED)
        && element.name == name
        && element.num_children.is_none()
        && element.converted_type == Some(ConvertedType::UINT_64)
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && matches!(
            element.logical_type,
            Some(LogicalType::INTEGER(ref integer))
                if integer.bit_width == 64 && !integer.is_signed
        )
}

fn validate_metadata(
    metadata: &FileMetaData,
    link: &PatchRegionLink,
) -> Result<(), MultiscaleColumnarError> {
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidMetadata))?;
    let expected = patch_region_metadata(SpatialPhysicalEncoding::Parquet, link);
    if entries.len() != expected.len() {
        return Err(parquet_failure(SpatialParquetFailure::InvalidMetadata));
    }
    let mut total = 0_usize;
    for (entry, (key, value)) in entries.iter().zip(expected) {
        let observed = entry
            .value
            .as_deref()
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidMetadata))?;
        total = total
            .checked_add(entry.key.len())
            .and_then(|sum| sum.checked_add(observed.len()))
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        if entry.key != key || observed != value || observed.is_empty() {
            return Err(parquet_failure(SpatialParquetFailure::InvalidMetadata));
        }
    }
    if total > MAXIMUM_APPLICATION_METADATA_BYTES {
        return Err(parquet_failure(SpatialParquetFailure::InvalidMetadata));
    }
    Ok(())
}

fn column_profile(index: usize) -> Result<(Type, &'static str), MultiscaleColumnarError> {
    match index {
        0 => Ok((Type::BYTE_ARRAY, "patch_id")),
        1 => Ok((Type::BYTE_ARRAY, "region_id")),
        2 => Ok((Type::BYTE_ARRAY, "relation")),
        3 => Ok((Type::INT64, "overlap_numerator")),
        4 => Ok((Type::INT64, "overlap_denominator")),
        _ => Err(parquet_failure(SpatialParquetFailure::InvalidRowGroup)),
    }
}

fn decoded_bytes(link: &PatchRegionLink) -> Result<u64, MultiscaleColumnarError> {
    link.nonzero_relations()
        .chunks(ROW_GROUP_ROWS)
        .try_fold(0_u64, |total, rows| {
            total
                .checked_add(
                    u64::try_from(patch_region_decoded_bytes(rows)?)
                        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
                )
                .ok_or(MultiscaleColumnarError::SizeOverflow)
        })
}

fn footer_compact_limits(expected_groups: usize) -> Result<CompactLimits, MultiscaleColumnarError> {
    let schema_elements = COLUMN_COUNT
        .checked_add(1)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let per_group_elements = COLUMN_COUNT
        .checked_mul(5)
        .and_then(|value| value.checked_add(1))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let top_level_elements = COLUMN_COUNT
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_add(PATCH_REGION_METADATA_KEYS.len()))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let maximum_total_elements = expected_groups
        .checked_mul(per_group_elements)
        .and_then(|value| value.checked_add(top_level_elements))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let maximum_fields = expected_groups
        .checked_mul(256)
        .and_then(|value| value.checked_add(256))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    Ok(CompactLimits {
        maximum_depth: 16,
        maximum_fields,
        maximum_collection_elements: expected_groups
            .max(PATCH_REGION_METADATA_KEYS.len())
            .max(schema_elements)
            .max(COLUMN_COUNT),
        maximum_total_elements,
        maximum_string_bytes: MAXIMUM_APPLICATION_METADATA_BYTES,
        maximum_total_string_bytes: MAXIMUM_FOOTER_BYTES,
    })
}

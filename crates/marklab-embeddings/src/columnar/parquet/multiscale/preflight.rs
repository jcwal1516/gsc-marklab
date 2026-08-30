use std::io::{Cursor, Read, Seek, SeekFrom};

use marklab_project::ContentDigest;
use parquet::{
    file::metadata::{ParquetMetaData, ParquetMetaDataReader},
    format::{
        ColumnOrder, ConvertedType, Encoding, FieldRepetitionType, FileMetaData, LogicalType,
        PageHeader, PageType, SchemaElement, Type,
    },
    thrift::TSerializable,
};

use crate::{ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph};

pub(super) use super::physical::parquet_failure;
use super::physical::{
    absolute_slice, page_limits, read_exact_at, validate_column_features, validate_footer_length,
};
use super::profile::{metadata_entries, SpatialParquetProfile};
use crate::columnar::{
    multiscale::{
        enforce_decoded_budget, enforce_file_budget, enforce_retained_budget,
        enforce_row_group_budget, validate_footprint_domain, validate_overlap_domain,
    },
    parquet::{
        compact::{is_canonical_compact, BoundedCompactProtocol},
        preflight::{estimate_raw_preflight_bytes, footer_compact_limits_with_metadata},
        profile::{
            CREATED_BY, MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_FOOTER_BYTES,
            MAXIMUM_PAGE_BYTES, MAXIMUM_PAGE_HEADER_BYTES, PARQUET_MAGIC, ROW_GROUP_ROWS,
            TRAILER_BYTES,
        },
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialParquetFailure,
};

const PAGE_ROWS: usize = 1_024;

macro_rules! preflight_summary {
    ($name:ident) => {
        #[doc = "Validated structural declaration for one canonical C-05 Parquet table."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name {
            row_count: u64,
            row_group_count: u32,
            encoded_byte_len: u64,
            content_digest: ContentDigest,
            pub(super) retained_preflight_bytes: usize,
        }

        impl $name {
            pub(super) fn from_common(common: &CommonParquetPreflight) -> Self {
                Self {
                    row_count: common.row_count,
                    row_group_count: common.row_group_count,
                    encoded_byte_len: common.encoded_byte_len,
                    content_digest: common.content_digest,
                    retained_preflight_bytes: common.retained_preflight_bytes,
                }
            }

            /// Exact canonical row count.
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
    };
}

preflight_summary!(PatchFootprintParquetPreflight);
preflight_summary!(PatchOverlapParquetPreflight);

pub(super) struct CommonParquetPreflight {
    pub(super) row_count: u64,
    pub(super) row_group_count: u32,
    pub(super) encoded_byte_len: u64,
    pub(super) content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
    pub(super) metadata: ParquetMetaData,
}

/// Raw-preflight borrowed canonical footprint Parquet bytes.
pub fn preflight_patch_footprint_set_parquet_bytes(
    bytes: &[u8],
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchFootprintParquetPreflight, MultiscaleColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    let prepared = prepare_reader(
        &mut Cursor::new(bytes),
        encoded,
        ContentDigest::from_bytes(bytes),
        SpatialParquetProfile::Footprint,
        expected,
        context,
        footprints,
        None,
        budgets,
    )?;
    Ok(PatchFootprintParquetPreflight::from_common(&prepared))
}

/// Raw-preflight borrowed canonical overlap Parquet bytes.
pub fn preflight_patch_overlap_graph_parquet_bytes(
    bytes: &[u8],
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchOverlapParquetPreflight, MultiscaleColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    let prepared = prepare_reader(
        &mut Cursor::new(bytes),
        encoded,
        ContentDigest::from_bytes(bytes),
        SpatialParquetProfile::Overlap,
        expected,
        context,
        footprints,
        Some(overlap),
        budgets,
    )?;
    Ok(PatchOverlapParquetPreflight::from_common(&prepared))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    profile: SpatialParquetProfile,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CommonParquetPreflight, MultiscaleColumnarError> {
    enforce_file_budget(encoded_byte_len, budgets)?;
    match profile {
        SpatialParquetProfile::Footprint => {
            validate_footprint_domain(expected, context, footprints)?
        }
        SpatialParquetProfile::Overlap => validate_overlap_domain(
            expected,
            context,
            footprints,
            overlap.ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?,
        )?,
    }
    let row_count = match profile {
        SpatialParquetProfile::Footprint => footprints.row_count(),
        SpatialParquetProfile::Overlap => overlap
            .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?
            .edge_count(),
    };
    enforce_decoded_budget(decoded_bytes(profile, footprints, overlap)?, budgets)?;
    let minimum = u64::try_from(PARQUET_MAGIC.len() + TRAILER_BYTES)
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    if encoded_byte_len < minimum {
        return Err(parquet_failure(SpatialParquetFailure::InvalidMagic));
    }
    let actual = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| parquet_failure(SpatialParquetFailure::ArtifactRead))?;
    if actual != encoded_byte_len {
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
    let metadata_count = match profile {
        SpatialParquetProfile::Footprint => 7,
        SpatialParquetProfile::Overlap => 8,
    };
    let limits = footer_compact_limits_with_metadata(expected_groups, metadata_count)
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
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
        profile,
        expected,
        footprints,
        overlap,
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
            validate_pages(
                &chunk,
                start,
                start,
                end,
                profile,
                column,
                group_start,
                rows,
                footprints,
                overlap,
            )
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
    Ok(CommonParquetPreflight {
        row_count: u64::try_from(row_count).map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        row_group_count: u32::try_from(row_group_count)
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_preflight_bytes: retained,
        metadata: stock,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_metadata_tree<F>(
    footer_start: usize,
    metadata: &FileMetaData,
    profile: SpatialParquetProfile,
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    budgets: EmbeddingColumnarBudgets,
    mut page_validator: F,
) -> Result<(), MultiscaleColumnarError>
where
    F: FnMut(usize, usize, usize, usize, usize) -> Result<usize, MultiscaleColumnarError>,
{
    let expected_rows = match profile {
        SpatialParquetProfile::Footprint => footprints.row_count(),
        SpatialParquetProfile::Overlap => overlap
            .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?
            .edge_count(),
    };
    if metadata.version != 2
        || usize::try_from(metadata.num_rows).ok() != Some(expected_rows)
        || metadata.row_groups.len() != expected_rows.div_ceil(ROW_GROUP_ROWS)
    {
        return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
    }
    validate_schema(&metadata.schema, profile)?;
    validate_metadata(metadata, profile, expected, footprints, overlap)?;
    if metadata.created_by.as_deref() != Some(CREATED_BY)
        || metadata.encryption_algorithm.is_some()
        || metadata.footer_signing_key_metadata.is_some()
        || metadata.column_orders.as_ref().is_none_or(|orders| {
            orders.len() != profile.column_count()
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
            || group.columns.len() != profile.column_count()
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
        let mut column_ranges = [(0_usize, 0_usize); 3];
        for (column_index, column) in group.columns.iter().enumerate() {
            let column_metadata = column
                .meta_data
                .as_ref()
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowGroup))?;
            validate_column_features(column, column_metadata)?;
            let (expected_type, expected_path) = column_profile(profile, column_index)?;
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
        for (column_index, (start, end)) in column_ranges
            .iter()
            .copied()
            .take(profile.column_count())
            .enumerate()
        {
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

fn validate_page_header_length(header_bytes: usize) -> Result<(), MultiscaleColumnarError> {
    if header_bytes == 0 || header_bytes > MAXIMUM_PAGE_HEADER_BYTES {
        return Err(parquet_failure(SpatialParquetFailure::InvalidPage));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_page_declarations(
    header: &PageHeader,
    body_bytes: usize,
    uncompressed: usize,
    page_end: usize,
    column_end: usize,
    page_start_row: usize,
    group_rows: usize,
) -> Result<usize, MultiscaleColumnarError> {
    let page = header
        .data_page_header_v2
        .as_ref()
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?;
    let rows = usize::try_from(page.num_rows)
        .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?;
    let values = usize::try_from(page.num_values)
        .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?;
    if body_bytes > MAXIMUM_PAGE_BYTES
        || body_bytes != uncompressed
        || page_end > column_end
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
        || values != rows
        || page.num_nulls != 0
        || page.definition_levels_byte_length != 0
        || page.repetition_levels_byte_length != 0
        || page_start_row
            .checked_add(rows)
            .is_none_or(|value| value > group_rows)
    {
        return Err(parquet_failure(SpatialParquetFailure::InvalidPage));
    }
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
fn validate_pages(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
    profile: SpatialParquetProfile,
    column_index: usize,
    group_start_row: usize,
    group_rows: usize,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
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
        validate_page_header_length(header_bytes)?;
        if !is_canonical_compact(
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
        let rows = validate_page_declarations(
            &header,
            body_bytes,
            uncompressed,
            page_end,
            end,
            page_start_row,
            group_rows,
        )?;
        let data = absolute_slice(bytes, base_offset, body_start, page_end)
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?;
        let absolute_start = group_start_row
            .checked_add(page_start_row)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        validate_plain_values(
            data,
            profile,
            column_index,
            absolute_start,
            rows,
            footprints,
            overlap,
        )?;
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

#[allow(clippy::too_many_arguments)]
fn validate_plain_values(
    bytes: &[u8],
    profile: SpatialParquetProfile,
    column: usize,
    start: usize,
    rows: usize,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> Result<(), MultiscaleColumnarError> {
    match profile {
        SpatialParquetProfile::Footprint => {
            let selected = footprints
                .footprints()
                .get(
                    start
                        ..start
                            .checked_add(rows)
                            .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                )
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?;
            match column {
                0 => validate_plain_strings(
                    bytes,
                    selected.iter().map(|row| row.patch_id().as_str()),
                ),
                1 => validate_plain_i64(bytes, selected.iter().map(|row| row.origin_px()[0])),
                2 => validate_plain_i64(bytes, selected.iter().map(|row| row.origin_px()[1])),
                _ => Err(parquet_failure(SpatialParquetFailure::InvalidPage)),
            }
        }
        SpatialParquetProfile::Overlap => {
            let selected = overlap
                .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?
                .edges()
                .get(
                    start
                        ..start
                            .checked_add(rows)
                            .ok_or(MultiscaleColumnarError::SizeOverflow)?,
                )
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?;
            match column {
                0 => validate_plain_strings(
                    bytes,
                    selected.iter().map(|row| row.left_patch_id().as_str()),
                ),
                1 => validate_plain_strings(
                    bytes,
                    selected.iter().map(|row| row.right_patch_id().as_str()),
                ),
                _ => Err(parquet_failure(SpatialParquetFailure::InvalidPage)),
            }
        }
    }
}

fn validate_plain_strings<'a>(
    bytes: &[u8],
    expected: impl Iterator<Item = &'a str>,
) -> Result<(), MultiscaleColumnarError> {
    let mut cursor = 0_usize;
    for value in expected {
        let length_end = cursor
            .checked_add(4)
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

fn validate_plain_i64(
    bytes: &[u8],
    expected: impl Iterator<Item = i64>,
) -> Result<(), MultiscaleColumnarError> {
    let mut count = 0_usize;
    for (index, value) in expected.enumerate() {
        let start = index
            .checked_mul(8)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let end = start
            .checked_add(8)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let observed = i64::from_le_bytes(
            bytes
                .get(start..end)
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidPage))?,
        );
        if observed != value {
            return Err(parquet_failure(SpatialParquetFailure::InvalidCanonicalRows));
        }
        count = index + 1;
    }
    if bytes.len()
        != count
            .checked_mul(8)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?
    {
        return Err(parquet_failure(SpatialParquetFailure::InvalidPage));
    }
    Ok(())
}

fn validate_schema(
    schema: &[SchemaElement],
    profile: SpatialParquetProfile,
) -> Result<(), MultiscaleColumnarError> {
    let valid = match profile {
        SpatialParquetProfile::Footprint => {
            schema.len() == 4
                && is_group(&schema[0], profile.root(), 3)
                && is_utf8(&schema[1], "patch_id")
                && is_i64(&schema[2], "origin_x_px")
                && is_i64(&schema[3], "origin_y_px")
        }
        SpatialParquetProfile::Overlap => {
            schema.len() == 3
                && is_group(&schema[0], profile.root(), 2)
                && is_utf8(&schema[1], "left_patch_id")
                && is_utf8(&schema[2], "right_patch_id")
        }
    };
    if !valid {
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

fn is_i64(element: &SchemaElement, name: &str) -> bool {
    element.type_ == Some(Type::INT64)
        && element.type_length.is_none()
        && element.repetition_type == Some(FieldRepetitionType::REQUIRED)
        && element.name == name
        && element.num_children.is_none()
        && element.converted_type.is_none()
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && element.logical_type.is_none()
}

fn validate_metadata(
    metadata: &FileMetaData,
    profile: SpatialParquetProfile,
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> Result<(), MultiscaleColumnarError> {
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidMetadata))?;
    let expected_entries = metadata_entries(profile, expected, footprints, overlap);
    if entries.len() != expected_entries.len() {
        return Err(parquet_failure(SpatialParquetFailure::InvalidMetadata));
    }
    let mut total = 0_usize;
    for (entry, (key, value)) in entries.iter().zip(expected_entries) {
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

fn column_profile(
    profile: SpatialParquetProfile,
    index: usize,
) -> Result<(Type, &'static str), MultiscaleColumnarError> {
    match (profile, index) {
        (SpatialParquetProfile::Footprint, 0) => Ok((Type::BYTE_ARRAY, "patch_id")),
        (SpatialParquetProfile::Footprint, 1) => Ok((Type::INT64, "origin_x_px")),
        (SpatialParquetProfile::Footprint, 2) => Ok((Type::INT64, "origin_y_px")),
        (SpatialParquetProfile::Overlap, 0) => Ok((Type::BYTE_ARRAY, "left_patch_id")),
        (SpatialParquetProfile::Overlap, 1) => Ok((Type::BYTE_ARRAY, "right_patch_id")),
        _ => Err(parquet_failure(SpatialParquetFailure::InvalidRowGroup)),
    }
}

fn decoded_bytes(
    profile: SpatialParquetProfile,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> Result<u64, MultiscaleColumnarError> {
    let mut total = 0_u64;
    match profile {
        SpatialParquetProfile::Footprint => {
            for rows in footprints.footprints().chunks(ROW_GROUP_ROWS) {
                let text = rows.iter().try_fold(0_usize, |sum, row| {
                    sum.checked_add(row.patch_id().as_str().len())
                        .ok_or(MultiscaleColumnarError::SizeOverflow)
                })?;
                total = total
                    .checked_add(
                        u64::try_from(decoded_chunk(rows.len(), 1, text, 2)?)
                            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
                    )
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            }
        }
        SpatialParquetProfile::Overlap => {
            for rows in overlap
                .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?
                .edges()
                .chunks(ROW_GROUP_ROWS)
            {
                let text = rows.iter().try_fold(0_usize, |sum, row| {
                    sum.checked_add(row.left_patch_id().as_str().len())
                        .and_then(|value| value.checked_add(row.right_patch_id().as_str().len()))
                        .ok_or(MultiscaleColumnarError::SizeOverflow)
                })?;
                total = total
                    .checked_add(
                        u64::try_from(decoded_chunk(rows.len(), 2, text, 0)?)
                            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?,
                    )
                    .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            }
        }
    }
    Ok(total)
}

fn decoded_chunk(
    rows: usize,
    utf8_columns: usize,
    text: usize,
    i64_columns: usize,
) -> Result<usize, MultiscaleColumnarError> {
    let validity = rows
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    validity
        .checked_mul(utf8_columns + i64_columns)
        .and_then(|value| {
            value.checked_add(
                rows.checked_add(1)?
                    .checked_mul(4)?
                    .checked_mul(utf8_columns)?,
            )
        })
        .and_then(|value| value.checked_add(text))
        .and_then(|value| value.checked_add(rows.checked_mul(8)?.checked_mul(i64_columns)?))
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use parquet::format::{
        ColumnChunk, ColumnMetaData, CompressionCodec, DataPageHeaderV2, PageEncodingStats,
        Statistics, StringType,
    };

    use super::*;

    fn footprint_schema_elements() -> Vec<SchemaElement> {
        vec![
            SchemaElement {
                type_: None,
                type_length: None,
                repetition_type: None,
                name: "marklab_patch_footprint_table".to_owned(),
                num_children: Some(3),
                converted_type: None,
                scale: None,
                precision: None,
                field_id: None,
                logical_type: None,
            },
            SchemaElement {
                type_: Some(Type::BYTE_ARRAY),
                type_length: None,
                repetition_type: Some(FieldRepetitionType::REQUIRED),
                name: "patch_id".to_owned(),
                num_children: None,
                converted_type: Some(ConvertedType::UTF8),
                scale: None,
                precision: None,
                field_id: None,
                logical_type: Some(LogicalType::STRING(StringType::new())),
            },
            SchemaElement {
                type_: Some(Type::INT64),
                type_length: None,
                repetition_type: Some(FieldRepetitionType::REQUIRED),
                name: "origin_x_px".to_owned(),
                num_children: None,
                converted_type: None,
                scale: None,
                precision: None,
                field_id: None,
                logical_type: None,
            },
            SchemaElement {
                type_: Some(Type::INT64),
                type_length: None,
                repetition_type: Some(FieldRepetitionType::REQUIRED),
                name: "origin_y_px".to_owned(),
                num_children: None,
                converted_type: None,
                scale: None,
                precision: None,
                field_id: None,
                logical_type: None,
            },
        ]
    }

    fn canonical_column() -> (ColumnChunk, ColumnMetaData) {
        let metadata = ColumnMetaData {
            type_: Type::BYTE_ARRAY,
            encodings: vec![Encoding::PLAIN, Encoding::RLE],
            path_in_schema: vec!["patch_id".to_owned()],
            codec: CompressionCodec::UNCOMPRESSED,
            num_values: 1,
            total_uncompressed_size: 8,
            total_compressed_size: 8,
            key_value_metadata: None,
            data_page_offset: 4,
            index_page_offset: None,
            dictionary_page_offset: None,
            statistics: None,
            encoding_stats: Some(vec![PageEncodingStats::new(
                PageType::DATA_PAGE_V2,
                Encoding::PLAIN,
                1,
            )]),
            bloom_filter_offset: None,
            bloom_filter_length: None,
            size_statistics: None,
            geospatial_statistics: None,
        };
        let column = ColumnChunk {
            file_path: None,
            file_offset: 0,
            meta_data: Some(metadata.clone()),
            offset_index_offset: None,
            offset_index_length: None,
            column_index_offset: None,
            column_index_length: None,
            crypto_metadata: None,
            encrypted_column_metadata: None,
        };
        (column, metadata)
    }

    fn canonical_page() -> PageHeader {
        PageHeader {
            type_: PageType::DATA_PAGE_V2,
            uncompressed_page_size: 8,
            compressed_page_size: 8,
            crc: None,
            data_page_header: None,
            index_page_header: None,
            dictionary_page_header: None,
            data_page_header_v2: Some(DataPageHeaderV2 {
                num_values: 1,
                num_nulls: 0,
                num_rows: 1,
                encoding: Encoding::PLAIN,
                definition_levels_byte_length: 0,
                repetition_levels_byte_length: 0,
                is_compressed: Some(false),
                statistics: None,
            }),
        }
    }

    #[test]
    fn c05_parquet_schema_rejects_annotation_and_extra_column_drift() {
        let canonical = footprint_schema_elements();
        validate_schema(&canonical, SpatialParquetProfile::Footprint)
            .expect("canonical footprint schema");

        let mut annotation = canonical.clone();
        annotation[1].logical_type = None;
        assert!(matches!(
            validate_schema(&annotation, SpatialParquetProfile::Footprint),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidSchema,
            })
        ));

        let mut extra = canonical;
        extra.push(extra[3].clone());
        assert!(matches!(
            validate_schema(&extra, SpatialParquetProfile::Footprint),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidSchema,
            })
        ));
    }

    #[test]
    fn c05_parquet_column_guards_reject_codec_dictionary_index_and_statistics() {
        let (column, metadata) = canonical_column();
        validate_column_features(&column, &metadata).expect("canonical column features");

        let mut compressed = metadata.clone();
        compressed.codec = CompressionCodec::SNAPPY;
        let mut dictionary = metadata.clone();
        dictionary.encodings.push(Encoding::RLE_DICTIONARY);
        let mut dictionary_page = metadata.clone();
        dictionary_page.dictionary_page_offset = Some(4);
        let mut statistics = metadata.clone();
        statistics.statistics = Some(Statistics::default());
        for hostile in [compressed, dictionary, dictionary_page, statistics] {
            assert!(matches!(
                validate_column_features(&column, &hostile),
                Err(MultiscaleColumnarError::Parquet {
                    reason: SpatialParquetFailure::ForbiddenFeature,
                })
            ));
        }

        let mut index = column;
        index.offset_index_offset = Some(4);
        index.offset_index_length = Some(8);
        assert!(matches!(
            validate_column_features(&index, &metadata),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::ForbiddenFeature,
            })
        ));
    }

    #[test]
    fn c05_parquet_footer_page_header_page_and_range_bounds_are_exact() {
        assert_eq!(
            validate_footer_length(MAXIMUM_FOOTER_BYTES, MAXIMUM_FOOTER_BYTES + 4)
                .expect("maximum footer"),
            4
        );
        assert!(validate_footer_length(MAXIMUM_FOOTER_BYTES + 1, usize::MAX).is_err());
        validate_page_header_length(MAXIMUM_PAGE_HEADER_BYTES).expect("maximum page header");
        assert!(validate_page_header_length(MAXIMUM_PAGE_HEADER_BYTES + 1).is_err());

        let canonical = canonical_page();
        assert_eq!(
            validate_page_declarations(&canonical, 8, 8, 24, 24, 0, 1).expect("canonical page"),
            1
        );
        assert!(validate_page_declarations(
            &canonical,
            MAXIMUM_PAGE_BYTES + 1,
            MAXIMUM_PAGE_BYTES + 1,
            MAXIMUM_PAGE_BYTES + 1,
            MAXIMUM_PAGE_BYTES + 1,
            0,
            1,
        )
        .is_err());
        assert!(validate_page_declarations(&canonical, 8, 8, 25, 24, 0, 1).is_err());

        let mut compressed = canonical.clone();
        compressed
            .data_page_header_v2
            .as_mut()
            .expect("data page")
            .is_compressed = Some(true);
        let mut statistics = canonical.clone();
        statistics
            .data_page_header_v2
            .as_mut()
            .expect("data page")
            .statistics = Some(Statistics::default());
        let mut crc = canonical;
        crc.crc = Some(1);
        for hostile in [compressed, statistics, crc] {
            assert!(validate_page_declarations(&hostile, 8, 8, 24, 24, 0, 1).is_err());
        }
    }
}

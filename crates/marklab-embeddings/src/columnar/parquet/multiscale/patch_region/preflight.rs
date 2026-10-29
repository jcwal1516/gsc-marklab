use crate::columnar::parquet::physical::{decode_stock_metadata, read_footer, RawFooter};
use crate::columnar::parquet::profile::MAXIMUM_FOOTER_BYTES;

use std::io::{Cursor, Read, Seek};

use marklab_project::ContentDigest;
use parquet::{
    file::metadata::ParquetMetaData,
    format::{ConvertedType, FieldRepetitionType, FileMetaData, LogicalType, SchemaElement, Type},
};

use crate::columnar::parquet::schema::{is_flat_group as is_group, is_required_utf8 as is_utf8};
use crate::PatchRegionLink;

pub(super) use super::super::super::physical::parquet_failure;
use super::super::super::physical::{
    read_data_page, read_exact_at, validate_fixed_width, validate_plain_strings,
    validate_plain_u64, validate_row_group_tree, validate_strings,
};
use super::profile::{COLUMN_COUNT, ROOT};
use crate::columnar::{
    multiscale::{
        enforce_decoded_budget, enforce_file_budget, enforce_retained_budget,
        patch_region_decoded_bytes, patch_region_metadata, validate_patch_region_domain,
        PATCH_REGION_METADATA_KEYS,
    },
    parquet::{
        compact::CompactLimits,
        profile::{MAXIMUM_APPLICATION_METADATA_BYTES, ROW_GROUP_ROWS},
    },
    ColumnarError, EmbeddingColumnarBudgets, ParquetFailure,
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
) -> Result<PatchRegionParquetPreflight, ColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
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
) -> Result<PreparedPatchRegionParquet, ColumnarError> {
    enforce_file_budget(encoded_byte_len, budgets)?;
    validate_patch_region_domain(link)?;
    let row_count = link.nonzero_relation_count();
    enforce_decoded_budget(decoded_bytes(link)?, budgets)?;
    let RawFooter {
        footer,
        raw,
        footer_start,
        raw_retained,
        stock_retained,
    } = read_footer(
        reader,
        encoded_byte_len,
        || footer_compact_limits(row_count.div_ceil(ROW_GROUP_ROWS)),
        budgets,
    )?;
    let mut retained = raw_retained.max(stock_retained);
    validate_metadata_tree(
        footer_start,
        &raw,
        link,
        budgets,
        |start, end, column, group_start, rows| {
            let chunk_len = end.checked_sub(start).ok_or(ColumnarError::SizeOverflow)?;
            let peak = raw_retained
                .checked_add(chunk_len)
                .ok_or(ColumnarError::SizeOverflow)?;
            retained = retained.max(peak);
            enforce_retained_budget(peak, budgets)?;
            let mut chunk = Vec::new();
            chunk
                .try_reserve_exact(chunk_len)
                .map_err(|_| ColumnarError::AllocationFailed {
                    requested: chunk_len,
                })?;
            chunk.resize(chunk_len, 0);
            read_exact_at(
                reader,
                u64::try_from(start).map_err(|_| ColumnarError::SizeOverflow)?,
                &mut chunk,
            )?;
            validate_pages(&chunk, start, start, end, column, group_start, rows, link)
        },
    )?;
    let row_group_count = raw.row_groups.len();
    drop(raw);
    let stock = decode_stock_metadata(&footer, row_count, stock_retained)?;
    Ok(PreparedPatchRegionParquet {
        row_count: u64::try_from(row_count).map_err(|_| ColumnarError::SizeOverflow)?,
        row_group_count: u32::try_from(row_group_count).map_err(|_| ColumnarError::SizeOverflow)?,
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
    page_validator: F,
) -> Result<(), ColumnarError>
where
    F: FnMut(usize, usize, usize, usize, usize) -> Result<usize, ColumnarError>,
{
    let expected_rows = link.nonzero_relation_count();
    if metadata.version != 2
        || usize::try_from(metadata.num_rows).ok() != Some(expected_rows)
        || metadata.row_groups.len() != expected_rows.div_ceil(ROW_GROUP_ROWS)
    {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    validate_schema(&metadata.schema)?;
    validate_metadata(metadata, link)?;
    validate_row_group_tree::<COLUMN_COUNT, _, _>(
        footer_start,
        metadata,
        expected_rows,
        COLUMN_COUNT,
        budgets,
        |column_index, rows, column_metadata| {
            let (expected_type, expected_path) = column_profile(column_index)?;
            if column_metadata.type_ != expected_type
                || column_metadata
                    .path_in_schema
                    .iter()
                    .map(String::as_str)
                    .ne(std::iter::once(expected_path))
                || usize::try_from(column_metadata.num_values).ok() != Some(rows)
            {
                return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
            }
            Ok(())
        },
        page_validator,
    )
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
) -> Result<usize, ColumnarError> {
    validate_pages_profile(
        bytes,
        base_offset,
        start,
        end,
        column_index,
        group_start_row,
        group_rows,
        Some(link),
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
    link: Option<&PatchRegionLink>,
) -> Result<usize, ColumnarError> {
    let mut cursor = start;
    let mut page_count = 0_usize;
    let mut page_start_row = 0_usize;
    while cursor < end {
        let physical =
            read_data_page(bytes, base_offset, cursor, end, ParquetFailure::InvalidPage)?;
        let page = physical
            .header
            .data_page_header_v2
            .as_ref()
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let rows = usize::try_from(page.num_rows)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        if rows == 0
            || rows != group_rows.saturating_sub(page_start_row).min(PAGE_ROWS)
            || usize::try_from(page.num_values).ok() != Some(rows)
            || page.num_nulls != 0
            || page.definition_levels_byte_length != 0
            || page.repetition_levels_byte_length != 0
            || page_start_row
                .checked_add(rows)
                .is_none_or(|value| value > group_rows)
        {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let body = physical.body;
        let absolute_start = group_start_row
            .checked_add(page_start_row)
            .ok_or(ColumnarError::SizeOverflow)?;
        if let Some(link) = link {
            validate_plain_values(body, column_index, absolute_start, rows, link)?;
        } else if column_index < 3 {
            validate_strings(body, rows)?;
        } else {
            validate_fixed_width(body, rows, 8)?;
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

fn validate_plain_values(
    bytes: &[u8],
    column: usize,
    start: usize,
    rows: usize,
    link: &PatchRegionLink,
) -> Result<(), ColumnarError> {
    let end = start.checked_add(rows).ok_or(ColumnarError::SizeOverflow)?;
    let selected = link
        .nonzero_relations()
        .get(start..end)
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
    match column {
        0 => validate_plain_strings(bytes, selected.iter().map(|row| row.patch_id().as_str())),
        1 => validate_plain_strings(bytes, selected.iter().map(|row| row.region_id().as_str())),
        2 => validate_plain_strings(bytes, selected.iter().map(|row| row.relation().wire_name())),
        3 => validate_plain_u64(bytes, selected.iter().map(|row| row.numerator())),
        4 => validate_plain_u64(bytes, selected.iter().map(|row| row.denominator())),
        _ => Err(parquet_failure(ParquetFailure::InvalidPage)),
    }
}

pub(in crate::columnar::parquet) fn validate_schema(
    schema: &[SchemaElement],
) -> Result<(), ColumnarError> {
    if schema.len() != COLUMN_COUNT + 1
        || !is_group(&schema[0], ROOT, COLUMN_COUNT as i32)
        || !is_utf8(&schema[1], "patch_id")
        || !is_utf8(&schema[2], "region_id")
        || !is_utf8(&schema[3], "relation")
        || !is_u64(&schema[4], "overlap_numerator")
        || !is_u64(&schema[5], "overlap_denominator")
    {
        return Err(parquet_failure(ParquetFailure::InvalidSchema));
    }
    Ok(())
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

fn validate_metadata(metadata: &FileMetaData, link: &PatchRegionLink) -> Result<(), ColumnarError> {
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
    let expected = patch_region_metadata(SpatialPhysicalEncoding::Parquet, link);
    if entries.len() != expected.len() {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    let mut total = 0_usize;
    for (entry, (key, value)) in entries.iter().zip(expected) {
        let observed = entry
            .value
            .as_deref()
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
        total = total
            .checked_add(entry.key.len())
            .and_then(|sum| sum.checked_add(observed.len()))
            .ok_or(ColumnarError::SizeOverflow)?;
        if entry.key != key || observed != value || observed.is_empty() {
            return Err(parquet_failure(ParquetFailure::InvalidMetadata));
        }
    }
    if total > MAXIMUM_APPLICATION_METADATA_BYTES {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn column_profile(
    index: usize,
) -> Result<(Type, &'static str), ColumnarError> {
    match index {
        0 => Ok((Type::BYTE_ARRAY, "patch_id")),
        1 => Ok((Type::BYTE_ARRAY, "region_id")),
        2 => Ok((Type::BYTE_ARRAY, "relation")),
        3 => Ok((Type::INT64, "overlap_numerator")),
        4 => Ok((Type::INT64, "overlap_denominator")),
        _ => Err(parquet_failure(ParquetFailure::InvalidRowGroup)),
    }
}

fn decoded_bytes(link: &PatchRegionLink) -> Result<u64, ColumnarError> {
    link.nonzero_relations()
        .chunks(ROW_GROUP_ROWS)
        .try_fold(0_u64, |total, rows| {
            total
                .checked_add(
                    u64::try_from(patch_region_decoded_bytes(rows)?)
                        .map_err(|_| ColumnarError::SizeOverflow)?,
                )
                .ok_or(ColumnarError::SizeOverflow)
        })
}

pub(in crate::columnar::parquet) fn footer_compact_limits(
    expected_groups: usize,
) -> Result<CompactLimits, ColumnarError> {
    let schema_elements = COLUMN_COUNT
        .checked_add(1)
        .ok_or(ColumnarError::SizeOverflow)?;
    let per_group_elements = COLUMN_COUNT
        .checked_mul(5)
        .and_then(|value| value.checked_add(1))
        .ok_or(ColumnarError::SizeOverflow)?;
    let top_level_elements = COLUMN_COUNT
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_add(PATCH_REGION_METADATA_KEYS.len()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let maximum_total_elements = expected_groups
        .checked_mul(per_group_elements)
        .and_then(|value| value.checked_add(top_level_elements))
        .ok_or(ColumnarError::SizeOverflow)?;
    let maximum_fields = expected_groups
        .checked_mul(256)
        .and_then(|value| value.checked_add(256))
        .ok_or(ColumnarError::SizeOverflow)?;
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

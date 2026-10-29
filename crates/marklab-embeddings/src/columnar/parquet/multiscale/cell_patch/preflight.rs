use crate::columnar::parquet::physical::{decode_stock_metadata, read_footer, RawFooter};
use crate::columnar::parquet::profile::MAXIMUM_FOOTER_BYTES;

use crate::columnar::parquet::levels::check_level_stream;
use crate::columnar::parquet::multiscale::preflight::preflight_summary;
use std::io::{Cursor, Read, Seek};

use marklab_project::ContentDigest;
use parquet::{
    file::metadata::ParquetMetaData,
    format::{FieldRepetitionType, FileMetaData, SchemaElement, Type},
};

use crate::columnar::parquet::schema::{
    is_flat_group as is_group, is_required_utf8 as is_utf8, is_unsigned_u64 as is_u64,
};
use crate::{CellPatchAssignmentMode, CellPatchLink};

pub(super) use super::super::super::physical::parquet_failure;
use super::super::super::physical::{
    level_error, read_data_page, read_exact_at, validate_fixed_width, validate_plain_strings,
    validate_plain_u64, validate_row_group_tree, validate_strings,
};
use super::profile::CellPatchParquetProfile;
use crate::columnar::{
    multiscale::{
        assignment_decoded_bytes, cell_patch_metadata, edge_decoded_bytes, enforce_decoded_budget,
        enforce_file_budget, enforce_retained_budget, validate_cell_patch_domain,
        CELL_PATCH_METADATA_KEYS,
    },
    parquet::{
        compact::CompactLimits,
        profile::{MAXIMUM_APPLICATION_METADATA_BYTES, ROW_GROUP_ROWS},
    },
    ColumnarError, EmbeddingColumnarBudgets, ParquetFailure,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

const PAGE_ROWS: usize = 1_024;

preflight_summary!(
    CellPatchAssignmentParquetPreflight,
    "Validated structural declaration for one canonical cell-patch Parquet table."
);
preflight_summary!(
    CellPatchEdgeParquetPreflight,
    "Validated structural declaration for one canonical cell-patch Parquet table."
);

pub(super) struct CommonParquetPreflight {
    pub(super) row_count: u64,
    pub(super) row_group_count: u32,
    pub(super) encoded_byte_len: u64,
    pub(super) content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
    pub(super) metadata: ParquetMetaData,
}

/// Raw-preflight borrowed canonical cell-patch assignment Parquet bytes.
pub fn preflight_cell_patch_assignment_table_parquet_bytes(
    bytes: &[u8],
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchAssignmentParquetPreflight, ColumnarError> {
    preflight_bytes(bytes, CellPatchParquetProfile::Assignment, link, budgets)
        .map(|common| CellPatchAssignmentParquetPreflight::from_common(&common))
}

/// Raw-preflight borrowed canonical cell-patch edge Parquet bytes.
pub fn preflight_cell_patch_edge_table_parquet_bytes(
    bytes: &[u8],
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchEdgeParquetPreflight, ColumnarError> {
    preflight_bytes(bytes, CellPatchParquetProfile::Edge, link, budgets)
        .map(|common| CellPatchEdgeParquetPreflight::from_common(&common))
}

fn preflight_bytes(
    bytes: &[u8],
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CommonParquetPreflight, ColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    prepare_reader(
        &mut Cursor::new(bytes),
        encoded,
        ContentDigest::from_bytes(bytes),
        profile,
        link,
        budgets,
    )
}

pub(super) fn prepare_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CommonParquetPreflight, ColumnarError> {
    enforce_file_budget(encoded_byte_len, budgets)?;
    validate_cell_patch_domain(link)?;
    let row_count = profile.row_count(link);
    enforce_decoded_budget(decoded_bytes(profile, link)?, budgets)?;
    let RawFooter {
        footer,
        raw,
        footer_start,
        raw_retained,
        stock_retained,
    } = read_footer(
        reader,
        encoded_byte_len,
        || footer_compact_limits(row_count.div_ceil(ROW_GROUP_ROWS), profile),
        budgets,
    )?;
    let mut retained = raw_retained.max(stock_retained);
    validate_metadata_tree(
        footer_start,
        &raw,
        profile,
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
            validate_pages(
                &chunk,
                start,
                start,
                end,
                profile,
                column,
                group_start,
                rows,
                link,
            )
        },
    )?;
    let row_group_count = raw.row_groups.len();
    drop(raw);
    let stock = decode_stock_metadata(&footer, row_count, stock_retained)?;
    Ok(CommonParquetPreflight {
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
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
    page_validator: F,
) -> Result<(), ColumnarError>
where
    F: FnMut(usize, usize, usize, usize, usize) -> Result<usize, ColumnarError>,
{
    let expected_rows = profile.row_count(link);
    if metadata.version != 2
        || usize::try_from(metadata.num_rows).ok() != Some(expected_rows)
        || metadata.row_groups.len() != expected_rows.div_ceil(ROW_GROUP_ROWS)
    {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    validate_schema(&metadata.schema, profile)?;
    validate_metadata(metadata, profile, link)?;
    validate_row_group_tree::<6, _, _>(
        footer_start,
        metadata,
        expected_rows,
        profile.column_count(),
        budgets,
        |column_index, rows, column_metadata| {
            let (expected_type, expected_path) = column_profile(profile, column_index)?;
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
    profile: CellPatchParquetProfile,
    column_index: usize,
    group_start_row: usize,
    group_rows: usize,
    link: &CellPatchLink,
) -> Result<usize, ColumnarError> {
    validate_pages_profile(
        bytes,
        base_offset,
        start,
        end,
        profile,
        column_index,
        group_start_row,
        group_rows,
        link.mode(),
        Some(link),
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::columnar::parquet) fn validate_pages_profile(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
    profile: CellPatchParquetProfile,
    column_index: usize,
    group_start_row: usize,
    group_rows: usize,
    mode: CellPatchAssignmentMode,
    link: Option<&CellPatchLink>,
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
        let values = usize::try_from(page.num_values)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let nulls = usize::try_from(page.num_nulls)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let definitions = usize::try_from(page.definition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let repetitions = usize::try_from(page.repetition_levels_byte_length)
            .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
        let optional = profile == CellPatchParquetProfile::Edge && column_index >= 2;
        let present = mode == CellPatchAssignmentMode::DeclaredWeightedInterpolation;
        let expected_nulls = if optional && !present { rows } else { 0 };
        if rows == 0
            || rows != group_rows.saturating_sub(page_start_row).min(PAGE_ROWS)
            || values != rows
            || nulls != expected_nulls
            || repetitions != 0
            || (optional && definitions == 0)
            || (!optional && definitions != 0)
            || page_start_row
                .checked_add(rows)
                .is_none_or(|value| value > group_rows)
        {
            return Err(parquet_failure(ParquetFailure::InvalidPage));
        }
        let body = physical.body;
        if optional {
            validate_optional_levels(
                body.get(..definitions)
                    .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?,
                rows,
                present,
            )?;
        }
        let data = body
            .get(definitions..)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
        let absolute_start = group_start_row
            .checked_add(page_start_row)
            .ok_or(ColumnarError::SizeOverflow)?;
        if let Some(link) = link {
            validate_plain_values(data, profile, column_index, absolute_start, rows, link)?;
        } else if (profile == CellPatchParquetProfile::Assignment && column_index < 2)
            || (profile == CellPatchParquetProfile::Edge && column_index == 1)
        {
            validate_strings(data, rows)?;
        } else {
            validate_fixed_width(data, rows - expected_nulls, 8)?;
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
    profile: CellPatchParquetProfile,
    column: usize,
    start: usize,
    rows: usize,
    link: &CellPatchLink,
) -> Result<(), ColumnarError> {
    let end = start.checked_add(rows).ok_or(ColumnarError::SizeOverflow)?;
    match profile {
        CellPatchParquetProfile::Assignment => {
            let selected = link
                .assignments()
                .get(start..end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            match column {
                0 => {
                    validate_plain_strings(bytes, selected.iter().map(|row| row.cell_id().as_str()))
                }
                1 => validate_plain_strings(
                    bytes,
                    selected.iter().map(|row| row.status().wire_name()),
                ),
                2 => validate_plain_u64(
                    bytes,
                    selected.iter().map(|row| row.anchor_px()[0].to_bits()),
                ),
                3 => validate_plain_u64(
                    bytes,
                    selected.iter().map(|row| row.anchor_px()[1].to_bits()),
                ),
                4 => validate_plain_u64(bytes, selected.iter().map(|row| row.edge_start())),
                5 => validate_plain_u64(bytes, selected.iter().map(|row| row.edge_count())),
                _ => Err(parquet_failure(ParquetFailure::InvalidPage)),
            }
        }
        CellPatchParquetProfile::Edge => {
            let selected = link
                .edges()
                .get(start..end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            match column {
                0 => validate_plain_u64(bytes, selected.iter().map(|row| row.assignment_row())),
                1 => validate_plain_strings(
                    bytes,
                    selected.iter().map(|row| row.patch_id().as_str()),
                ),
                2 => validate_plain_u64(
                    bytes,
                    selected
                        .iter()
                        .filter_map(|row| row.weight().map(|weight| weight.numerator())),
                ),
                3 => validate_plain_u64(
                    bytes,
                    selected
                        .iter()
                        .filter_map(|row| row.weight().map(|weight| weight.denominator())),
                ),
                _ => Err(parquet_failure(ParquetFailure::InvalidPage)),
            }
        }
    }
}

fn validate_optional_levels(
    bytes: &[u8],
    expected_rows: usize,
    expected_present: bool,
) -> Result<(), ColumnarError> {
    check_level_stream(bytes, expected_rows, |_| Some(u8::from(expected_present)))
        .map_err(level_error)
}

pub(in crate::columnar::parquet) fn validate_schema(
    schema: &[SchemaElement],
    profile: CellPatchParquetProfile,
) -> Result<(), ColumnarError> {
    let valid = match profile {
        CellPatchParquetProfile::Assignment => {
            schema.len() == 7
                && is_group(&schema[0], profile.root(), 6)
                && is_utf8(&schema[1], "cell_id")
                && is_utf8(&schema[2], "assignment_status")
                && is_u64(&schema[3], "anchor_x_bits", FieldRepetitionType::REQUIRED)
                && is_u64(&schema[4], "anchor_y_bits", FieldRepetitionType::REQUIRED)
                && is_u64(&schema[5], "edge_start", FieldRepetitionType::REQUIRED)
                && is_u64(&schema[6], "edge_count", FieldRepetitionType::REQUIRED)
        }
        CellPatchParquetProfile::Edge => {
            schema.len() == 5
                && is_group(&schema[0], profile.root(), 4)
                && is_u64(&schema[1], "assignment_row", FieldRepetitionType::REQUIRED)
                && is_utf8(&schema[2], "patch_id")
                && is_u64(
                    &schema[3],
                    "weight_numerator",
                    FieldRepetitionType::OPTIONAL,
                )
                && is_u64(
                    &schema[4],
                    "weight_denominator",
                    FieldRepetitionType::OPTIONAL,
                )
        }
    };
    if !valid {
        return Err(parquet_failure(ParquetFailure::InvalidSchema));
    }
    Ok(())
}

fn validate_metadata(
    metadata: &FileMetaData,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
) -> Result<(), ColumnarError> {
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
    let expected = cell_patch_metadata(profile.role(), SpatialPhysicalEncoding::Parquet, link);
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
    profile: CellPatchParquetProfile,
    index: usize,
) -> Result<(Type, &'static str), ColumnarError> {
    match (profile, index) {
        (CellPatchParquetProfile::Assignment, 0) => Ok((Type::BYTE_ARRAY, "cell_id")),
        (CellPatchParquetProfile::Assignment, 1) => Ok((Type::BYTE_ARRAY, "assignment_status")),
        (CellPatchParquetProfile::Assignment, 2) => Ok((Type::INT64, "anchor_x_bits")),
        (CellPatchParquetProfile::Assignment, 3) => Ok((Type::INT64, "anchor_y_bits")),
        (CellPatchParquetProfile::Assignment, 4) => Ok((Type::INT64, "edge_start")),
        (CellPatchParquetProfile::Assignment, 5) => Ok((Type::INT64, "edge_count")),
        (CellPatchParquetProfile::Edge, 0) => Ok((Type::INT64, "assignment_row")),
        (CellPatchParquetProfile::Edge, 1) => Ok((Type::BYTE_ARRAY, "patch_id")),
        (CellPatchParquetProfile::Edge, 2) => Ok((Type::INT64, "weight_numerator")),
        (CellPatchParquetProfile::Edge, 3) => Ok((Type::INT64, "weight_denominator")),
        _ => Err(parquet_failure(ParquetFailure::InvalidRowGroup)),
    }
}

fn decoded_bytes(
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
) -> Result<u64, ColumnarError> {
    let mut chunks: Box<dyn Iterator<Item = Result<usize, ColumnarError>> + '_> = match profile {
        CellPatchParquetProfile::Assignment => Box::new(
            link.assignments()
                .chunks(ROW_GROUP_ROWS)
                .map(assignment_decoded_bytes),
        ),
        CellPatchParquetProfile::Edge => {
            Box::new(link.edges().chunks(ROW_GROUP_ROWS).map(edge_decoded_bytes))
        }
    };
    chunks.try_fold(0_u64, |total, bytes| {
        total
            .checked_add(u64::try_from(bytes?).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)
    })
}

pub(in crate::columnar::parquet) fn footer_compact_limits(
    expected_groups: usize,
    profile: CellPatchParquetProfile,
) -> Result<CompactLimits, ColumnarError> {
    let columns = profile.column_count();
    let schema_elements = columns.checked_add(1).ok_or(ColumnarError::SizeOverflow)?;
    let per_group_elements = columns
        .checked_mul(5)
        .and_then(|value| value.checked_add(1))
        .ok_or(ColumnarError::SizeOverflow)?;
    let top_level_elements = columns
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_add(CELL_PATCH_METADATA_KEYS.len()))
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
            .max(CELL_PATCH_METADATA_KEYS.len())
            .max(schema_elements)
            .max(columns),
        maximum_total_elements,
        maximum_string_bytes: MAXIMUM_APPLICATION_METADATA_BYTES,
        maximum_total_string_bytes: MAXIMUM_FOOTER_BYTES,
    })
}

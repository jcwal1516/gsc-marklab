use crate::columnar::parquet::physical::{decode_stock_metadata, read_footer, RawFooter};

use std::io::{Cursor, Read, Seek};

use marklab_project::ContentDigest;
use parquet::{
    file::metadata::ParquetMetaData,
    format::{
        Encoding, FieldRepetitionType, FileMetaData, PageHeader, PageType, SchemaElement, Type,
    },
};

use crate::columnar::parquet::schema::{is_flat_group as is_group, is_required_utf8 as is_utf8};
use crate::{ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph};

pub(super) use super::super::physical::parquet_failure;
use super::super::physical::{
    read_data_page, read_exact_at, validate_fixed_width, validate_plain_strings,
    validate_row_group_tree, validate_strings,
};
use super::profile::{metadata_entries, SpatialParquetProfile};
use crate::columnar::{
    multiscale::{
        enforce_decoded_budget, enforce_file_budget, enforce_retained_budget,
        validate_footprint_domain, validate_overlap_domain,
    },
    parquet::{
        preflight::footer_compact_limits_with_metadata,
        profile::{MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_PAGE_BYTES, ROW_GROUP_ROWS},
    },
    ColumnarError, EmbeddingColumnarBudgets, ParquetFailure,
};

const PAGE_ROWS: usize = 1_024;

macro_rules! preflight_summary {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
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

pub(super) use preflight_summary;

preflight_summary!(
    PatchFootprintParquetPreflight,
    "Validated structural declaration for one canonical multiscale Parquet table."
);
preflight_summary!(
    PatchOverlapParquetPreflight,
    "Validated structural declaration for one canonical multiscale Parquet table."
);

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
) -> Result<PatchFootprintParquetPreflight, ColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
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
) -> Result<PatchOverlapParquetPreflight, ColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
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
) -> Result<CommonParquetPreflight, ColumnarError> {
    enforce_file_budget(encoded_byte_len, budgets)?;
    match profile {
        SpatialParquetProfile::Footprint => {
            validate_footprint_domain(expected, context, footprints)?
        }
        SpatialParquetProfile::Overlap => validate_overlap_domain(
            expected,
            context,
            footprints,
            overlap.ok_or(ColumnarError::ArtifactBindingMismatch)?,
        )?,
    }
    let row_count = match profile {
        SpatialParquetProfile::Footprint => footprints.row_count(),
        SpatialParquetProfile::Overlap => overlap
            .ok_or(ColumnarError::ArtifactBindingMismatch)?
            .edge_count(),
    };
    enforce_decoded_budget(decoded_bytes(profile, footprints, overlap)?, budgets)?;
    let RawFooter {
        footer,
        raw,
        footer_start,
        raw_retained,
        stock_retained,
    } = read_footer(
        reader,
        encoded_byte_len,
        || {
            let metadata_count = match profile {
                SpatialParquetProfile::Footprint => 7,
                SpatialParquetProfile::Overlap => 8,
            };
            footer_compact_limits_with_metadata(row_count.div_ceil(ROW_GROUP_ROWS), metadata_count)
                .map_err(|_| ColumnarError::SizeOverflow)
        },
        budgets,
    )?;
    let mut retained = raw_retained.max(stock_retained);
    validate_metadata_tree(
        footer_start,
        &raw,
        profile,
        expected,
        footprints,
        overlap,
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
                footprints,
                overlap,
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

#[allow(clippy::too_many_arguments)]
fn validate_metadata_tree<F>(
    footer_start: usize,
    metadata: &FileMetaData,
    profile: SpatialParquetProfile,
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    budgets: EmbeddingColumnarBudgets,
    page_validator: F,
) -> Result<(), ColumnarError>
where
    F: FnMut(usize, usize, usize, usize, usize) -> Result<usize, ColumnarError>,
{
    let expected_rows = match profile {
        SpatialParquetProfile::Footprint => footprints.row_count(),
        SpatialParquetProfile::Overlap => overlap
            .ok_or(ColumnarError::ArtifactBindingMismatch)?
            .edge_count(),
    };
    if metadata.version != 2
        || usize::try_from(metadata.num_rows).ok() != Some(expected_rows)
        || metadata.row_groups.len() != expected_rows.div_ceil(ROW_GROUP_ROWS)
    {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    validate_schema(&metadata.schema, profile)?;
    validate_metadata(metadata, profile, expected, footprints, overlap)?;
    validate_row_group_tree::<3, _, _>(
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

fn validate_page_declarations(
    header: &PageHeader,
    body_bytes: usize,
    uncompressed: usize,
    page_end: usize,
    column_end: usize,
    page_start_row: usize,
    group_rows: usize,
) -> Result<usize, ColumnarError> {
    let page = header
        .data_page_header_v2
        .as_ref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
    let rows =
        usize::try_from(page.num_rows).map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
    let values = usize::try_from(page.num_values)
        .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?;
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
        return Err(parquet_failure(ParquetFailure::InvalidPage));
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
        Some(footprints),
        overlap,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::columnar::parquet) fn validate_pages_profile(
    bytes: &[u8],
    base_offset: usize,
    start: usize,
    end: usize,
    profile: SpatialParquetProfile,
    column_index: usize,
    group_start_row: usize,
    group_rows: usize,
    footprints: Option<&PatchFootprintSet>,
    overlap: Option<&PatchOverlapGraph>,
) -> Result<usize, ColumnarError> {
    let mut cursor = start;
    let mut page_count = 0_usize;
    let mut page_start_row = 0_usize;
    while cursor < end {
        let physical =
            read_data_page(bytes, base_offset, cursor, end, ParquetFailure::InvalidPage)?;
        let rows = validate_page_declarations(
            &physical.header,
            physical.body.len(),
            physical.body.len(),
            physical.end,
            end,
            page_start_row,
            group_rows,
        )?;
        let data = physical.body;
        let absolute_start = group_start_row
            .checked_add(page_start_row)
            .ok_or(ColumnarError::SizeOverflow)?;
        if let Some(footprints) = footprints {
            validate_plain_values(
                data,
                profile,
                column_index,
                absolute_start,
                rows,
                footprints,
                overlap,
            )?;
        } else if profile == SpatialParquetProfile::Overlap || column_index == 0 {
            validate_strings(data, rows)?;
        } else {
            validate_fixed_width(data, rows, 8)?;
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
    profile: SpatialParquetProfile,
    column: usize,
    start: usize,
    rows: usize,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> Result<(), ColumnarError> {
    match profile {
        SpatialParquetProfile::Footprint => {
            let selected = footprints
                .footprints()
                .get(start..start.checked_add(rows).ok_or(ColumnarError::SizeOverflow)?)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            match column {
                0 => validate_plain_strings(
                    bytes,
                    selected.iter().map(|row| row.patch_id().as_str()),
                ),
                1 => validate_plain_i64(bytes, selected.iter().map(|row| row.origin_px()[0])),
                2 => validate_plain_i64(bytes, selected.iter().map(|row| row.origin_px()[1])),
                _ => Err(parquet_failure(ParquetFailure::InvalidPage)),
            }
        }
        SpatialParquetProfile::Overlap => {
            let selected = overlap
                .ok_or(ColumnarError::ArtifactBindingMismatch)?
                .edges()
                .get(start..start.checked_add(rows).ok_or(ColumnarError::SizeOverflow)?)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?;
            match column {
                0 => validate_plain_strings(
                    bytes,
                    selected.iter().map(|row| row.left_patch_id().as_str()),
                ),
                1 => validate_plain_strings(
                    bytes,
                    selected.iter().map(|row| row.right_patch_id().as_str()),
                ),
                _ => Err(parquet_failure(ParquetFailure::InvalidPage)),
            }
        }
    }
}

fn validate_plain_i64(
    bytes: &[u8],
    expected: impl Iterator<Item = i64>,
) -> Result<(), ColumnarError> {
    let mut count = 0_usize;
    for (index, value) in expected.enumerate() {
        let start = index.checked_mul(8).ok_or(ColumnarError::SizeOverflow)?;
        let end = start.checked_add(8).ok_or(ColumnarError::SizeOverflow)?;
        let observed = i64::from_le_bytes(
            bytes
                .get(start..end)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| parquet_failure(ParquetFailure::InvalidPage))?,
        );
        if observed != value {
            return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
        }
        count = index + 1;
    }
    if bytes.len() != count.checked_mul(8).ok_or(ColumnarError::SizeOverflow)? {
        return Err(parquet_failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn validate_schema(
    schema: &[SchemaElement],
    profile: SpatialParquetProfile,
) -> Result<(), ColumnarError> {
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
        return Err(parquet_failure(ParquetFailure::InvalidSchema));
    }
    Ok(())
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
) -> Result<(), ColumnarError> {
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
    let expected_entries = metadata_entries(profile, expected, footprints, overlap);
    if entries.len() != expected_entries.len() {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    let mut total = 0_usize;
    for (entry, (key, value)) in entries.iter().zip(expected_entries) {
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
    profile: SpatialParquetProfile,
    index: usize,
) -> Result<(Type, &'static str), ColumnarError> {
    match (profile, index) {
        (SpatialParquetProfile::Footprint, 0) => Ok((Type::BYTE_ARRAY, "patch_id")),
        (SpatialParquetProfile::Footprint, 1) => Ok((Type::INT64, "origin_x_px")),
        (SpatialParquetProfile::Footprint, 2) => Ok((Type::INT64, "origin_y_px")),
        (SpatialParquetProfile::Overlap, 0) => Ok((Type::BYTE_ARRAY, "left_patch_id")),
        (SpatialParquetProfile::Overlap, 1) => Ok((Type::BYTE_ARRAY, "right_patch_id")),
        _ => Err(parquet_failure(ParquetFailure::InvalidRowGroup)),
    }
}

fn decoded_bytes(
    profile: SpatialParquetProfile,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
) -> Result<u64, ColumnarError> {
    let mut total = 0_u64;
    match profile {
        SpatialParquetProfile::Footprint => {
            for rows in footprints.footprints().chunks(ROW_GROUP_ROWS) {
                let text = rows.iter().try_fold(0_usize, |sum, row| {
                    sum.checked_add(row.patch_id().as_str().len())
                        .ok_or(ColumnarError::SizeOverflow)
                })?;
                total = total
                    .checked_add(
                        u64::try_from(decoded_chunk(rows.len(), 1, text, 2)?)
                            .map_err(|_| ColumnarError::SizeOverflow)?,
                    )
                    .ok_or(ColumnarError::SizeOverflow)?;
            }
        }
        SpatialParquetProfile::Overlap => {
            for rows in overlap
                .ok_or(ColumnarError::ArtifactBindingMismatch)?
                .edges()
                .chunks(ROW_GROUP_ROWS)
            {
                let text = rows.iter().try_fold(0_usize, |sum, row| {
                    sum.checked_add(row.left_patch_id().as_str().len())
                        .and_then(|value| value.checked_add(row.right_patch_id().as_str().len()))
                        .ok_or(ColumnarError::SizeOverflow)
                })?;
                total = total
                    .checked_add(
                        u64::try_from(decoded_chunk(rows.len(), 2, text, 0)?)
                            .map_err(|_| ColumnarError::SizeOverflow)?,
                    )
                    .ok_or(ColumnarError::SizeOverflow)?;
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
) -> Result<usize, ColumnarError> {
    let validity = rows
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(ColumnarError::SizeOverflow)?;
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
        .ok_or(ColumnarError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use crate::columnar::parquet::{
        physical::{page_header_length_is_valid, validate_footer_length},
        profile::{MAXIMUM_FOOTER_BYTES, MAXIMUM_PAGE_HEADER_BYTES},
    };
    use parquet::format::{
        ColumnChunk, ColumnMetaData, CompressionCodec, ConvertedType, DataPageHeaderV2,
        LogicalType, PageEncodingStats, Statistics, StringType,
    };

    use super::super::super::physical::validate_column_features;
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
    fn multiscale_parquet_schema_rejects_annotation_and_extra_column_drift() {
        let canonical = footprint_schema_elements();
        validate_schema(&canonical, SpatialParquetProfile::Footprint)
            .expect("canonical footprint schema");

        let mut annotation = canonical.clone();
        annotation[1].logical_type = None;
        assert!(matches!(
            validate_schema(&annotation, SpatialParquetProfile::Footprint),
            Err(ColumnarError::Parquet {
                reason: ParquetFailure::InvalidSchema,
            })
        ));

        let mut extra = canonical;
        extra.push(extra[3].clone());
        assert!(matches!(
            validate_schema(&extra, SpatialParquetProfile::Footprint),
            Err(ColumnarError::Parquet {
                reason: ParquetFailure::InvalidSchema,
            })
        ));
    }

    #[test]
    fn multiscale_parquet_column_guards_reject_codec_dictionary_index_and_statistics() {
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
                Err(ColumnarError::Parquet {
                    reason: ParquetFailure::ForbiddenFeature,
                })
            ));
        }

        let mut index = column;
        index.offset_index_offset = Some(4);
        index.offset_index_length = Some(8);
        assert!(matches!(
            validate_column_features(&index, &metadata),
            Err(ColumnarError::Parquet {
                reason: ParquetFailure::ForbiddenFeature,
            })
        ));
    }

    #[test]
    fn multiscale_parquet_footer_page_header_page_and_range_bounds_are_exact() {
        assert_eq!(
            validate_footer_length(MAXIMUM_FOOTER_BYTES, MAXIMUM_FOOTER_BYTES + 4)
                .expect("maximum footer"),
            4
        );
        assert!(validate_footer_length(MAXIMUM_FOOTER_BYTES + 1, usize::MAX).is_err());
        assert!(page_header_length_is_valid(MAXIMUM_PAGE_HEADER_BYTES));
        assert!(!page_header_length_is_valid(MAXIMUM_PAGE_HEADER_BYTES + 1));

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

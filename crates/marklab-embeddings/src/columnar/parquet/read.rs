//! Reconstruction uses the same bounded page validators as physical verification.

use std::{
    io::{Read, Seek, SeekFrom},
    sync::Arc,
};

use arrow::{array::RecordBatchReader, datatypes::Schema, record_batch::RecordBatch};
use bytes::Bytes;
use marklab_project::{ArtifactRecord, TableColumnType, TableScalarType};
use parquet::{
    arrow::arrow_reader::{
        ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
    },
    file::metadata::{ParquetMetaData, ParquetMetaDataReader, RowGroupMetaData},
    format::{FileMetaData, SchemaElement, Type},
    thrift::TSerializable,
};

use crate::columnar::{
    enforce_decoded_budget, enforce_file_budget, enforce_retained_budget, enforce_row_group_budget,
    multiscale::{
        MatrixPhysicalProfile, CELL_PATCH_METADATA_KEYS, MATRIX_METADATA_KEYS,
        PATCH_REGION_METADATA_KEYS,
    },
    ColumnarError, EmbeddingColumnarBudgets, ParquetFailure,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;
use crate::{CellPatchAssignmentMode, EmbeddingStatus};

use super::{
    compact::{is_canonical_compact, BoundedCompactProtocol, CompactLimits},
    multiscale::{
        self, cell_patch, matrix, patch_region,
        physical::{validate_footer_length, validate_row_group_tree, validated_stock_group_range},
        profile::SpatialParquetProfile,
    },
    preflight::{estimate_raw_preflight_bytes, footer_compact_limits_with_metadata, read_exact_at},
    profile::{
        CREATED_BY, MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_FOOTER_BYTES, MAXIMUM_ROWS,
        PARQUET_MAGIC, ROW_GROUP_ROWS, TRAILER_BYTES,
    },
    reader::RowGroupWindow,
    row_link,
};

mod entities;
pub(crate) use entities::*;

#[derive(Clone, Copy)]
pub(super) enum ReadProfile<'a> {
    RowLink(&'a [EmbeddingStatus]),
    Footprint,
    Overlap,
    Assignment(CellPatchAssignmentMode),
    Edge(CellPatchAssignmentMode),
    PatchRegion,
    Matrix(MatrixPhysicalProfile, u32),
}

impl ReadProfile<'_> {
    fn keys(self) -> &'static [&'static str] {
        match self {
            Self::RowLink(_) => &row_link::profile::METADATA_KEYS,
            Self::Footprint => &multiscale::profile::FOOTPRINT_METADATA_KEYS,
            Self::Overlap => &multiscale::profile::OVERLAP_METADATA_KEYS,
            Self::Assignment(_) | Self::Edge(_) => &CELL_PATCH_METADATA_KEYS,
            Self::PatchRegion => &PATCH_REGION_METADATA_KEYS,
            Self::Matrix(_, _) => &MATRIX_METADATA_KEYS,
        }
    }

    pub(super) fn columns(self) -> usize {
        match self {
            Self::Overlap => 2,
            Self::Assignment(_) => 6,
            Self::Edge(_) => 4,
            Self::PatchRegion => 5,
            _ => 3,
        }
    }

    fn limits(self, groups: usize) -> Result<CompactLimits, ColumnarError> {
        match self {
            Self::Assignment(_) => cell_patch::preflight::footer_compact_limits(
                groups,
                cell_patch::profile::CellPatchParquetProfile::Assignment,
            ),
            Self::Edge(_) => cell_patch::preflight::footer_compact_limits(
                groups,
                cell_patch::profile::CellPatchParquetProfile::Edge,
            ),
            Self::PatchRegion => patch_region::preflight::footer_compact_limits(groups),
            Self::Matrix(_, _) => matrix::preflight::footer_compact_limits(groups),
            _ => footer_compact_limits_with_metadata(groups, self.keys().len()),
        }
    }

    fn validate_schema(self, schema: &[SchemaElement]) -> Result<(), ColumnarError> {
        match self {
            Self::RowLink(_) => row_link::preflight::schema::validate_schema(schema),
            Self::Footprint => {
                multiscale::preflight::validate_schema(schema, SpatialParquetProfile::Footprint)
            }
            Self::Overlap => {
                multiscale::preflight::validate_schema(schema, SpatialParquetProfile::Overlap)
            }
            Self::Assignment(_) => cell_patch::preflight::validate_schema(
                schema,
                cell_patch::profile::CellPatchParquetProfile::Assignment,
            ),
            Self::Edge(_) => cell_patch::preflight::validate_schema(
                schema,
                cell_patch::profile::CellPatchParquetProfile::Edge,
            ),
            Self::PatchRegion => patch_region::preflight::validate_schema(schema),
            Self::Matrix(profile, _) => matrix::preflight::validate_schema_profile(schema, profile),
        }
    }

    pub(super) fn schema(self) -> Result<Schema, ColumnarError> {
        Ok(match self {
            Self::RowLink(_) => row_link::profile::row_link_schema(),
            Self::Footprint => multiscale::profile::footprint_schema(),
            Self::Overlap => multiscale::profile::overlap_schema(),
            Self::Assignment(_) => cell_patch::profile::assignment_schema(),
            Self::Edge(_) => cell_patch::profile::edge_schema(),
            Self::PatchRegion => patch_region::profile::schema(),
            Self::Matrix(profile, dimension) => {
                Schema::new(crate::columnar::multiscale::matrix_schema_fields(
                    profile,
                    dimension,
                    SpatialPhysicalEncoding::Parquet,
                )?)
            }
        })
    }

    fn column(self, index: usize) -> Result<(Type, Vec<&'static str>), ColumnarError> {
        let (kind, path) = match self {
            Self::RowLink(_) => match index {
                0 => (Type::BYTE_ARRAY, "cell_id"),
                1 => (Type::INT64, "source_cell_row"),
                2 => (Type::INT64, "source_embedding_row"),
                _ => return Err(failure(ParquetFailure::InvalidSchema)),
            },
            Self::Footprint => {
                multiscale::preflight::column_profile(SpatialParquetProfile::Footprint, index)?
            }
            Self::Overlap => {
                multiscale::preflight::column_profile(SpatialParquetProfile::Overlap, index)?
            }
            Self::Assignment(_) => cell_patch::preflight::column_profile(
                cell_patch::profile::CellPatchParquetProfile::Assignment,
                index,
            )?,
            Self::Edge(_) => cell_patch::preflight::column_profile(
                cell_patch::profile::CellPatchParquetProfile::Edge,
                index,
            )?,
            Self::PatchRegion => patch_region::preflight::column_profile(index)?,
            Self::Matrix(profile, _) => {
                let (kind, path) = matrix::preflight::column_profile_for(profile, index)?;
                return Ok((kind, path.to_vec()));
            }
        };
        Ok((kind, vec![path]))
    }

    fn values(self, column: usize, rows: usize) -> Result<usize, ColumnarError> {
        match self {
            Self::Matrix(_, dimension) if column == 1 => rows
                .checked_mul(dimension as usize)
                .ok_or(ColumnarError::SizeOverflow),
            _ => Ok(rows),
        }
    }

    fn pages(
        self,
        bytes: &[u8],
        start: usize,
        end: usize,
        column: usize,
        group_start: usize,
        rows: usize,
    ) -> Result<usize, ColumnarError> {
        match self {
            Self::RowLink(statuses) => row_link::preflight::pages::validate_pages_profile(
                bytes,
                start,
                start,
                end,
                column,
                group_start,
                rows,
                None,
                Some(statuses),
            ),
            Self::Footprint => multiscale::preflight::validate_pages_profile(
                bytes,
                start,
                start,
                end,
                SpatialParquetProfile::Footprint,
                column,
                group_start,
                rows,
                None,
                None,
            ),
            Self::Overlap => multiscale::preflight::validate_pages_profile(
                bytes,
                start,
                start,
                end,
                SpatialParquetProfile::Overlap,
                column,
                group_start,
                rows,
                None,
                None,
            ),
            Self::Assignment(mode) => cell_patch::preflight::validate_pages_profile(
                bytes,
                start,
                start,
                end,
                cell_patch::profile::CellPatchParquetProfile::Assignment,
                column,
                group_start,
                rows,
                mode,
                None,
            ),
            Self::Edge(mode) => cell_patch::preflight::validate_pages_profile(
                bytes,
                start,
                start,
                end,
                cell_patch::profile::CellPatchParquetProfile::Edge,
                column,
                group_start,
                rows,
                mode,
                None,
            ),
            Self::PatchRegion => patch_region::preflight::validate_pages_profile(
                bytes,
                start,
                start,
                end,
                column,
                group_start,
                rows,
                None,
            ),
            Self::Matrix(_, dimension) => matrix::preflight::validate_pages_profile(
                bytes,
                start,
                start,
                end,
                column,
                group_start,
                rows,
                dimension,
                None,
            ),
        }
    }
}

pub(super) struct PreparedRead {
    pub(super) metadata: ParquetMetaData,
    pub(super) retained_preflight_bytes: usize,
    pub(super) collection_bytes: usize,
}

pub(super) fn manifest_rows(record: &ArtifactRecord) -> Result<usize, ColumnarError> {
    let rows = usize::try_from(
        record
            .table()
            .ok_or(ColumnarError::ArtifactBindingMismatch)?
            .row_count(),
    )
    .map_err(|_| ColumnarError::SizeOverflow)?;
    if rows > MAXIMUM_ROWS {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(rows)
}

pub(super) fn matrix_dimension(record: &ArtifactRecord) -> Result<u32, ColumnarError> {
    match record
        .table()
        .and_then(|table| table.columns().get(1))
        .map(|column| column.column_type())
    {
        Some(TableColumnType::FixedSizeList {
            element: TableScalarType::F32,
            length,
        }) if *length > 0 && *length <= 65_536 => Ok(*length),
        _ => Err(ColumnarError::ArtifactBindingMismatch),
    }
}

/// Profile validation finishes before metadata or pages enter the stock decoder.
pub(super) fn prepare<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    profile: ReadProfile<'_>,
    rows: usize,
    row_storage_bytes: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PreparedRead, ColumnarError> {
    let encoded = record.content().byte_len();
    enforce_file_budget(encoded, budgets)?;
    if rows > MAXIMUM_ROWS || manifest_rows(record)? != rows {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    if encoded < (PARQUET_MAGIC.len() + TRAILER_BYTES) as u64 {
        return Err(failure(ParquetFailure::InvalidMagic));
    }
    if reader
        .seek(SeekFrom::End(0))
        .map_err(|_| failure(ParquetFailure::ArtifactRead))?
        != encoded
    {
        return Err(failure(ParquetFailure::ArtifactRead));
    }
    let mut leading = [0; 4];
    read_exact_at(reader, 0, &mut leading)?;
    let mut trailer = [0; TRAILER_BYTES];
    read_exact_at(reader, encoded - TRAILER_BYTES as u64, &mut trailer)?;
    if &leading != PARQUET_MAGIC || trailer.get(4..) != Some(PARQUET_MAGIC) {
        return Err(failure(ParquetFailure::InvalidMagic));
    }
    let footer_len = u32::from_le_bytes(
        trailer[..4]
            .try_into()
            .map_err(|_| failure(ParquetFailure::InvalidFooterLength))?,
    ) as usize;
    let footer_start = validate_footer_length(
        footer_len,
        usize::try_from(encoded).map_err(|_| ColumnarError::SizeOverflow)? - TRAILER_BYTES,
    )?;
    let groups = rows.div_ceil(ROW_GROUP_ROWS);
    let limits = profile.limits(groups)?;
    let raw_retained = estimate_raw_preflight_bytes(footer_len, limits)?;
    let stock_retained = estimate_raw_preflight_bytes(MAXIMUM_FOOTER_BYTES, limits)?
        .checked_mul(2)
        .ok_or(ColumnarError::SizeOverflow)?;
    let mut retained = raw_retained.max(stock_retained);
    enforce_retained_budget(retained, budgets)?;
    let mut footer = allocate_bytes(footer_len)?;
    read_exact_at(reader, footer_start as u64, &mut footer)?;
    let mut protocol = BoundedCompactProtocol::new(&footer, limits);
    let raw = FileMetaData::read_from_in_protocol(&mut protocol)
        .map_err(|_| failure(ParquetFailure::InvalidFooter))?;
    if protocol
        .consumed_bytes()
        .map_err(|_| failure(ParquetFailure::InvalidFooter))?
        != footer.len()
        || !is_canonical_compact(&raw, &footer)
            .map_err(|_| failure(ParquetFailure::InvalidFooter))?
    {
        return Err(failure(ParquetFailure::InvalidFooter));
    }
    if raw.version != 2
        || usize::try_from(raw.num_rows).ok() != Some(rows)
        || raw.row_groups.len() != groups
    {
        return Err(failure(ParquetFailure::InvalidRowCount));
    }
    profile.validate_schema(&raw.schema)?;
    let entries = raw
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| failure(ParquetFailure::InvalidMetadata))?;
    if entries.len() != profile.keys().len() {
        return Err(failure(ParquetFailure::InvalidMetadata));
    }
    let mut metadata_bytes = 0_usize;
    for (entry, key) in entries.iter().zip(profile.keys()) {
        let value = entry
            .value
            .as_deref()
            .ok_or_else(|| failure(ParquetFailure::InvalidMetadata))?;
        if entry.key != *key || value.is_empty() {
            return Err(failure(ParquetFailure::InvalidMetadata));
        }
        metadata_bytes = metadata_bytes
            .checked_add(entry.key.len())
            .and_then(|n| n.checked_add(value.len()))
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    if metadata_bytes > MAXIMUM_APPLICATION_METADATA_BYTES {
        return Err(failure(ParquetFailure::InvalidMetadata));
    }
    let mut decoded = 0_usize;
    validate_row_group_tree::<6, _, _>(
        footer_start,
        &raw,
        rows,
        profile.columns(),
        budgets,
        |column, rows, metadata| {
            let (kind, path) = profile.column(column)?;
            if metadata.type_ != kind
                || metadata.path_in_schema.iter().map(String::as_str).ne(path)
                || usize::try_from(metadata.num_values).ok() != Some(profile.values(column, rows)?)
            {
                return Err(failure(ParquetFailure::InvalidRowGroup));
            }
            Ok(())
        },
        |start, end, column, group_start, rows| {
            let length = end.checked_sub(start).ok_or(ColumnarError::SizeOverflow)?;
            let peak = raw_retained
                .checked_add(length)
                .ok_or(ColumnarError::SizeOverflow)?;
            retained = retained.max(peak);
            enforce_retained_budget(peak, budgets)?;
            // PLAIN payload bounds owned scalar/string buffers; offsets and validity need extra space.
            let column_decoded = length
                .checked_add(
                    rows.checked_add(1)
                        .and_then(|n| n.checked_mul(16))
                        .ok_or(ColumnarError::SizeOverflow)?,
                )
                .ok_or(ColumnarError::SizeOverflow)?;
            decoded = decoded
                .checked_add(column_decoded)
                .ok_or(ColumnarError::SizeOverflow)?;
            enforce_decoded_budget(
                u64::try_from(decoded).map_err(|_| ColumnarError::SizeOverflow)?,
                budgets,
            )?;
            let mut chunk = allocate_bytes(length)?;
            read_exact_at(reader, start as u64, &mut chunk)?;
            profile.pages(&chunk, start, end, column, group_start, rows)
        },
    )?;
    drop(raw);
    let metadata = ParquetMetaDataReader::decode_metadata(&footer)
        .map_err(|_| failure(ParquetFailure::StockDecode))?;
    if metadata.memory_size() > stock_retained
        || metadata.file_metadata().version() != 2
        || usize::try_from(metadata.file_metadata().num_rows()).ok() != Some(rows)
        || metadata.num_row_groups() != groups
        || metadata.file_metadata().created_by() != Some(CREATED_BY)
        || metadata.column_index().is_some()
        || metadata.offset_index().is_some()
    {
        return Err(failure(ParquetFailure::StockDecode));
    }
    let collection_bytes = rows
        .checked_mul(row_storage_bytes)
        .and_then(|n| n.checked_add(footer_start))
        .ok_or(ColumnarError::SizeOverflow)?;
    Ok(PreparedRead {
        metadata,
        retained_preflight_bytes: retained,
        collection_bytes,
    })
}

pub(super) fn match_metadata(
    metadata: &ParquetMetaData,
    expected: impl IntoIterator<Item = (String, String)>,
) -> Result<(), ColumnarError> {
    let actual = metadata
        .file_metadata()
        .key_value_metadata()
        .ok_or_else(|| failure(ParquetFailure::InvalidMetadata))?;
    let mut expected = expected.into_iter();
    for entry in actual {
        let (key, value) = expected
            .next()
            .ok_or_else(|| failure(ParquetFailure::InvalidMetadata))?;
        if entry.key != key || entry.value.as_deref() != Some(value.as_str()) {
            return Err(failure(ParquetFailure::InvalidMetadata));
        }
    }
    if expected.next().is_some() {
        return Err(failure(ParquetFailure::InvalidMetadata));
    }
    Ok(())
}

pub(super) fn group_peak(group: &RowGroupMetaData) -> Result<usize, ColumnarError> {
    let (_, encoded) = validated_stock_group_range(group, group.num_columns())?;
    let rows = usize::try_from(group.num_rows()).map_err(|_| ColumnarError::SizeOverflow)?;
    // Retain the encoded window and decoder buffers, plus Arrow values, offsets and validity.
    encoded
        .checked_mul(6)
        .and_then(|n| {
            rows.checked_add(1)
                .and_then(|rows| rows.checked_mul(group.num_columns()))
                .and_then(|nrows| nrows.checked_mul(32))
                .and_then(|arrays| n.checked_add(arrays))
        })
        .and_then(|n| rows.checked_mul(4).and_then(|levels| n.checked_add(levels)))
        .ok_or(ColumnarError::SizeOverflow)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn visit_groups<R: Read + Seek + ?Sized>(
    source: &mut R,
    metadata: &ParquetMetaData,
    schema: Schema,
    rows: usize,
    retained_preflight: usize,
    held: usize,
    budgets: EmbeddingColumnarBudgets,
    mut visit: impl FnMut(&RecordBatch, &mut usize) -> Result<(), ColumnarError>,
) -> Result<(), ColumnarError> {
    let maximum_group = metadata
        .row_groups()
        .iter()
        .try_fold(0_usize, |maximum, group| {
            Ok::<_, ColumnarError>(maximum.max(group_peak(group)?))
        })?;
    let required = metadata
        .memory_size()
        .checked_mul(2)
        .and_then(|n| n.checked_add(maximum_group))
        .and_then(|n| n.checked_add(held))
        .and_then(|n| n.checked_add(64 * 1024))
        .ok_or(ColumnarError::SizeOverflow)?;
    enforce_retained_budget(required.max(retained_preflight), budgets)?;
    let metadata = Arc::new(metadata.clone());
    let reader_metadata = ArrowReaderMetadata::try_new(
        Arc::clone(&metadata),
        ArrowReaderOptions::new().with_schema(Arc::new(schema.clone())),
    )
    .map_err(|_| failure(ParquetFailure::StockDecode))?;
    let mut global = 0_usize;
    for (index, group) in metadata.row_groups().iter().enumerate() {
        let (start, length) = validated_stock_group_range(group, group.num_columns())?;
        enforce_row_group_budget(group_peak(group)?, budgets)?;
        let mut bytes = allocate_bytes(length)?;
        read_exact_at(source, start, &mut bytes)?;
        let end = start
            .checked_add(length as u64)
            .ok_or(ColumnarError::SizeOverflow)?;
        let window = RowGroupWindow::new(start, end, Bytes::from(bytes));
        let mut reader =
            ParquetRecordBatchReaderBuilder::new_with_metadata(window, reader_metadata.clone())
                .with_row_groups(vec![index])
                .with_batch_size(ROW_GROUP_ROWS)
                .build()
                .map_err(|_| failure(ParquetFailure::StockDecode))?;
        if reader.schema().as_ref() != &schema {
            return Err(failure(ParquetFailure::StockDecode));
        }
        let before = global;
        for batch in &mut reader {
            visit(
                &batch.map_err(|_| failure(ParquetFailure::StockDecode))?,
                &mut global,
            )?;
        }
        if global.checked_sub(before) != usize::try_from(group.num_rows()).ok() {
            return Err(failure(ParquetFailure::InvalidRowCount));
        }
    }
    if global != rows {
        return Err(failure(ParquetFailure::InvalidRowCount));
    }
    Ok(())
}

fn collect_rows<R: Read + Seek + ?Sized, T>(
    reader: &mut R,
    record: &ArtifactRecord,
    profile: ReadProfile<'_>,
    rows: usize,
    budgets: EmbeddingColumnarBudgets,
    mut collect: impl FnMut(&RecordBatch, &mut usize, &mut Vec<T>) -> Result<(), ColumnarError>,
) -> Result<(PreparedRead, Vec<T>), ColumnarError> {
    let prepared = prepare(reader, record, profile, rows, size_of::<T>(), budgets)?;
    collection_budget(&prepared, budgets)?;
    let mut values = crate::columnar::read_context::try_capacity(rows)?;
    visit_groups(
        reader,
        &prepared.metadata,
        profile.schema()?,
        rows,
        prepared.retained_preflight_bytes,
        prepared.collection_bytes,
        budgets,
        |batch, index| collect(batch, index, &mut values),
    )?;
    if values.len() != rows {
        return Err(failure(ParquetFailure::InvalidRowCount));
    }
    Ok((prepared, values))
}

fn collection_budget(
    prepared: &PreparedRead,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), ColumnarError> {
    let group = prepared
        .metadata
        .row_groups()
        .iter()
        .try_fold(0_usize, |maximum, group| {
            Ok::<_, ColumnarError>(maximum.max(group_peak(group)?))
        })?;
    let required = prepared
        .metadata
        .memory_size()
        .checked_mul(2)
        .and_then(|n| n.checked_add(group))
        .and_then(|n| n.checked_add(prepared.collection_bytes))
        .and_then(|n| n.checked_add(64 * 1024))
        .ok_or(ColumnarError::SizeOverflow)?;
    enforce_retained_budget(required.max(prepared.retained_preflight_bytes), budgets)
}

pub(super) fn validate_strings(bytes: &[u8], rows: usize) -> Result<(), ColumnarError> {
    let mut cursor = 0_usize;
    for _ in 0..rows {
        let prefix_end = cursor.checked_add(4).ok_or(ColumnarError::SizeOverflow)?;
        let length = u32::from_le_bytes(
            bytes
                .get(cursor..prefix_end)
                .ok_or_else(|| failure(ParquetFailure::InvalidPage))?
                .try_into()
                .map_err(|_| failure(ParquetFailure::InvalidPage))?,
        ) as usize;
        let end = prefix_end
            .checked_add(length)
            .ok_or(ColumnarError::SizeOverflow)?;
        std::str::from_utf8(
            bytes
                .get(prefix_end..end)
                .ok_or_else(|| failure(ParquetFailure::InvalidPage))?,
        )
        .map_err(|_| failure(ParquetFailure::InvalidCanonicalRows))?;
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

pub(super) fn validate_fixed_width(
    bytes: &[u8],
    count: usize,
    width: usize,
) -> Result<(), ColumnarError> {
    if bytes.len()
        != count
            .checked_mul(width)
            .ok_or(ColumnarError::SizeOverflow)?
    {
        return Err(failure(ParquetFailure::InvalidPage));
    }
    Ok(())
}

pub(super) fn validate_f32(bytes: &[u8], count: usize) -> Result<(), ColumnarError> {
    validate_fixed_width(bytes, count, 4)?;
    for chunk in bytes.as_chunks::<4>().0 {
        let bits = u32::from_le_bytes(*chunk);
        let value = f32::from_bits(bits);
        if !value.is_finite() || (value == 0.0 && bits != 0) {
            return Err(failure(ParquetFailure::InvalidCanonicalRows));
        }
    }
    Ok(())
}

fn allocate_bytes(length: usize) -> Result<Vec<u8>, ColumnarError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ColumnarError::AllocationFailed { requested: length })?;
    bytes.resize(length, 0);
    Ok(bytes)
}

pub(super) fn validate_batch_arrays(
    batch: &RecordBatch,
    columns: usize,
) -> Result<(), ColumnarError> {
    if batch.num_columns() != columns {
        return Err(failure(ParquetFailure::StockDecode));
    }
    for column in batch.columns() {
        column
            .to_data()
            .validate_full()
            .map_err(|_| failure(ParquetFailure::StockDecode))?;
    }
    Ok(())
}

pub(super) fn failure(reason: ParquetFailure) -> ColumnarError {
    ColumnarError::Parquet { reason }
}

use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    sync::Arc,
};

use arrow::array::{Array, RecordBatchReader, StringArray, UInt64Array};
use bytes::Bytes;
use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};
use parquet::{
    arrow::arrow_reader::{
        ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
    },
    file::metadata::RowGroupMetaData,
};

use crate::{
    CellPatchAssignmentMode, CellPatchLink, VerifiedCellPatchAssignmentArtifact,
    VerifiedCellPatchEdgeArtifact, VerifiedCellPatchInputArtifactGraph,
};

use super::{
    super::physical::validated_stock_group_range,
    preflight::{
        parquet_failure, prepare_reader, CellPatchAssignmentParquetPreflight,
        CellPatchEdgeParquetPreflight, CommonParquetPreflight,
    },
    profile::{expected_schema, CellPatchParquetProfile},
};
use crate::{
    columnar::{
        multiscale::{
            assignment_decoded_bytes, cell_patch_dependencies, edge_decoded_bytes,
            enforce_file_budget, enforce_retained_budget, enforce_row_group_budget,
        },
        parquet::{profile::ROW_GROUP_ROWS, reader::RowGroupWindow},
        EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialParquetFailure,
    },
    multiscale::physical::{record_matches_encoding, SpatialPhysicalEncoding},
};

/// Fully decode and validate borrowed canonical cell-patch assignment Parquet bytes.
pub fn validate_cell_patch_assignment_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchAssignmentParquetPreflight, MultiscaleColumnarError> {
    validate_borrowed(
        bytes,
        record,
        CellPatchParquetProfile::Assignment,
        link,
        budgets,
        CellPatchAssignmentParquetPreflight::from_common,
    )
}

/// Fully decode and validate borrowed canonical cell-patch edge Parquet bytes.
pub fn validate_cell_patch_edge_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchEdgeParquetPreflight, MultiscaleColumnarError> {
    validate_borrowed(
        bytes,
        record,
        CellPatchParquetProfile::Edge,
        link,
        budgets,
        CellPatchEdgeParquetPreflight::from_common,
    )
}

/// Fully decode one managed cell-patch assignment Parquet artifact in one integrity envelope.
pub fn validate_cell_patch_assignment_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchAssignmentParquetPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        validate_managed(
            reader,
            record,
            CellPatchParquetProfile::Assignment,
            link,
            budgets,
            CellPatchAssignmentParquetPreflight::from_common,
        )
    })
}

/// Fully decode one managed cell-patch edge Parquet artifact in one integrity envelope.
pub fn validate_cell_patch_edge_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellPatchEdgeParquetPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        validate_managed(
            reader,
            record,
            CellPatchParquetProfile::Edge,
            link,
            budgets,
            CellPatchEdgeParquetPreflight::from_common,
        )
    })
}

/// Verify borrowed assignment Parquet bytes and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_assignment_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchAssignmentArtifact, MultiscaleColumnarError> {
    validate_cell_patch_assignment_table_parquet_bytes(bytes, record, link, budgets)?;
    VerifiedCellPatchAssignmentArtifact::new(record.id(), link, graph)
}

/// Verify a managed assignment Parquet artifact and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_assignment_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchAssignmentArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_cell_patch_assignment_table_parquet_from_store(store, record, link, budgets)?;
    VerifiedCellPatchAssignmentArtifact::new(record.id(), link, graph)
        .map_err(VerifiedReaderError::Callback)
}

/// Verify borrowed edge Parquet bytes and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_edge_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchEdgeArtifact, MultiscaleColumnarError> {
    validate_cell_patch_edge_table_parquet_bytes(bytes, record, link, budgets)?;
    VerifiedCellPatchEdgeArtifact::new(record.id(), link, graph)
}

/// Verify a managed edge Parquet artifact and mint a graph-bound runtime receipt.
pub fn verify_cell_patch_edge_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellPatchEdgeArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_cell_patch_edge_table_parquet_from_store(store, record, link, budgets)?;
    VerifiedCellPatchEdgeArtifact::new(record.id(), link, graph)
        .map_err(VerifiedReaderError::Callback)
}

fn validate_borrowed<T>(
    bytes: &[u8],
    record: &ArtifactRecord,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
    summarize: fn(&CommonParquetPreflight) -> T,
) -> Result<T, MultiscaleColumnarError> {
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, profile, link)?;
    let mut source = Cursor::new(bytes);
    let prepared = prepare_reader(
        &mut source,
        encoded_len,
        marklab_project::ContentDigest::from_bytes(bytes),
        profile,
        link,
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    let summary = summarize(&prepared);
    decode_parquet(&mut source, profile, link, prepared, budgets)?;
    Ok(summary)
}

fn validate_managed<R: Read + Seek + ?Sized, T>(
    reader: &mut R,
    record: &ArtifactRecord,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
    summarize: fn(&CommonParquetPreflight) -> T,
) -> Result<T, MultiscaleColumnarError> {
    let encoded_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| parquet_failure(SpatialParquetFailure::ArtifactRead))?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, profile, link)?;
    let prepared = prepare_reader(
        reader,
        encoded_len,
        record.content().digest(),
        profile,
        link,
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    let summary = summarize(&prepared);
    decode_parquet(reader, profile, link, prepared, budgets)?;
    Ok(summary)
}

fn decode_parquet<R: Read + Seek + ?Sized>(
    source: &mut R,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    prepared: CommonParquetPreflight,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    let maximum_group_peak = maximum_group_peak(&prepared.metadata, profile, link)?;
    let retained = prepared
        .metadata
        .memory_size()
        .checked_add(maximum_group_peak)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?
        .max(prepared.retained_preflight_bytes);
    enforce_retained_budget(retained, budgets)?;

    let metadata = Arc::new(prepared.metadata);
    let schema = expected_schema(profile);
    let reader_metadata = ArrowReaderMetadata::try_new(
        Arc::clone(&metadata),
        ArrowReaderOptions::new().with_schema(Arc::new(schema.clone())),
    )
    .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
    let mut global_row = 0_usize;
    for (group_index, group) in metadata.row_groups().iter().enumerate() {
        let rows = usize::try_from(group.num_rows())
            .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
        let (start, length) = validated_stock_group_range(group, profile.column_count())?;
        let peak = estimate_group_peak(group, profile, link, global_row)?;
        enforce_row_group_budget(peak, budgets)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| MultiscaleColumnarError::AllocationFailed { requested: length })?;
        bytes.resize(length, 0);
        source
            .seek(SeekFrom::Start(start))
            .and_then(|_| source.read_exact(&mut bytes))
            .map_err(|_| parquet_failure(SpatialParquetFailure::ArtifactRead))?;
        let end = start
            .checked_add(u64::try_from(length).map_err(|_| MultiscaleColumnarError::SizeOverflow)?)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let window = RowGroupWindow::new(start, end, Bytes::from(bytes));
        let mut reader =
            ParquetRecordBatchReaderBuilder::new_with_metadata(window, reader_metadata.clone())
                .with_row_groups(vec![group_index])
                .with_batch_size(ROW_GROUP_ROWS)
                .build()
                .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
        if reader.schema().as_ref() != &schema {
            return Err(parquet_failure(SpatialParquetFailure::StockDecode));
        }
        let group_start = global_row;
        for decoded in &mut reader {
            let batch = decoded.map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
            validate_batch(&batch, profile, link, &mut global_row)?;
        }
        if global_row
            .checked_sub(group_start)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?
            != rows
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
        }
    }
    if global_row != profile.row_count(link) {
        return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
    }
    Ok(())
}

fn maximum_group_peak(
    metadata: &parquet::file::metadata::ParquetMetaData,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
) -> Result<usize, MultiscaleColumnarError> {
    let mut maximum = 0_usize;
    let mut group_start = 0_usize;
    for group in metadata.row_groups() {
        maximum = maximum.max(estimate_group_peak(group, profile, link, group_start)?);
        group_start = group_start
            .checked_add(
                usize::try_from(group.num_rows())
                    .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?,
            )
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    if group_start != profile.row_count(link) {
        return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
    }
    Ok(maximum)
}

fn validate_batch(
    batch: &arrow::record_batch::RecordBatch,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    global_row: &mut usize,
) -> Result<(), MultiscaleColumnarError> {
    if batch.num_columns() != profile.column_count() {
        return Err(parquet_failure(SpatialParquetFailure::StockDecode));
    }
    for column in batch.columns() {
        column
            .to_data()
            .validate_full()
            .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
    }
    match profile {
        CellPatchParquetProfile::Assignment => validate_assignment_batch(batch, link, global_row),
        CellPatchParquetProfile::Edge => validate_edge_batch(batch, link, global_row),
    }
}

fn validate_assignment_batch(
    batch: &arrow::record_batch::RecordBatch,
    link: &CellPatchLink,
    global_row: &mut usize,
) -> Result<(), MultiscaleColumnarError> {
    let cells = string_column(batch, 0)?;
    let statuses = string_column(batch, 1)?;
    let x = u64_column(batch, 2)?;
    let y = u64_column(batch, 3)?;
    let starts = u64_column(batch, 4)?;
    let counts = u64_column(batch, 5)?;
    if [
        cells.null_count(),
        statuses.null_count(),
        x.null_count(),
        y.null_count(),
        starts.null_count(),
        counts.null_count(),
    ]
    .iter()
    .any(|count| *count != 0)
    {
        return Err(parquet_failure(SpatialParquetFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        let row = link
            .assignments()
            .get(*global_row)
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowCount))?;
        if cells.value(local) != row.cell_id().as_str()
            || statuses.value(local) != row.status().wire_name()
            || x.value(local) != row.anchor_px()[0].to_bits()
            || y.value(local) != row.anchor_px()[1].to_bits()
            || starts.value(local) != row.edge_start()
            || counts.value(local) != row.edge_count()
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidCanonicalRows));
        }
        *global_row = global_row
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn validate_edge_batch(
    batch: &arrow::record_batch::RecordBatch,
    link: &CellPatchLink,
    global_row: &mut usize,
) -> Result<(), MultiscaleColumnarError> {
    let assignment_rows = u64_column(batch, 0)?;
    let patches = string_column(batch, 1)?;
    let numerators = u64_column(batch, 2)?;
    let denominators = u64_column(batch, 3)?;
    if assignment_rows.null_count() != 0 || patches.null_count() != 0 {
        return Err(parquet_failure(SpatialParquetFailure::StockDecode));
    }
    let expected_nulls = match link.mode() {
        CellPatchAssignmentMode::ContainedShared => batch.num_rows(),
        CellPatchAssignmentMode::DeclaredWeightedInterpolation => 0,
    };
    if numerators.null_count() != expected_nulls || denominators.null_count() != expected_nulls {
        return Err(parquet_failure(SpatialParquetFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        let row = link
            .edges()
            .get(*global_row)
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowCount))?;
        let weight_matches = match row.weight() {
            Some(weight) => {
                !numerators.is_null(local)
                    && numerators.value(local) == weight.numerator()
                    && !denominators.is_null(local)
                    && denominators.value(local) == weight.denominator()
            }
            None => numerators.is_null(local) && denominators.is_null(local),
        };
        if assignment_rows.value(local) != row.assignment_row()
            || patches.value(local) != row.patch_id().as_str()
            || !weight_matches
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidCanonicalRows));
        }
        *global_row = global_row
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn estimate_group_peak(
    group: &RowGroupMetaData,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
    group_start: usize,
) -> Result<usize, MultiscaleColumnarError> {
    let (_, encoded) = validated_stock_group_range(group, profile.column_count())?;
    let rows = usize::try_from(group.num_rows())
        .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
    let group_end = group_start
        .checked_add(rows)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let decoded = match profile {
        CellPatchParquetProfile::Assignment => assignment_decoded_bytes(
            link.assignments()
                .get(group_start..group_end)
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowCount))?,
        )?,
        CellPatchParquetProfile::Edge => edge_decoded_bytes(
            link.edges()
                .get(group_start..group_end)
                .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowCount))?,
        )?,
    };
    encoded
        .checked_mul(2)
        .and_then(|value| value.checked_add(decoded.checked_mul(2)?))
        .and_then(|value| value.checked_add(rows.checked_mul(4)?))
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

fn string_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&StringArray, MultiscaleColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::StockDecode))
}

fn u64_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&UInt64Array, MultiscaleColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::StockDecode))
}

fn validate_record(
    record: &ArtifactRecord,
    encoded_len: u64,
    profile: CellPatchParquetProfile,
    link: &CellPatchLink,
) -> Result<(), MultiscaleColumnarError> {
    let row_count = u64::try_from(profile.row_count(link))
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    if !record_matches_encoding(
        record,
        profile.role(),
        SpatialPhysicalEncoding::Parquet,
        row_count,
    ) || record.dependencies() != cell_patch_dependencies(link)
        || record.content().byte_len() != encoded_len
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    prepared: &CommonParquetPreflight,
    record: &ArtifactRecord,
) -> Result<(), MultiscaleColumnarError> {
    if prepared.content_digest != record.content().digest()
        || prepared.encoded_byte_len != record.content().byte_len()
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

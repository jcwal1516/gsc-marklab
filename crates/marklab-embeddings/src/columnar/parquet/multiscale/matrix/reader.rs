use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    sync::Arc,
};

use arrow::array::{Array, FixedSizeListArray, Float32Array, RecordBatchReader, StringArray};
use bytes::Bytes;
use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};
use parquet::{
    arrow::arrow_reader::{
        ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
    },
    file::metadata::RowGroupMetaData,
};

use crate::{
    DerivedRegionEmbeddingTableCandidate, DerivedSlideEmbeddingTableCandidate, EmbeddingStatus,
    PatchEmbeddingSourceRowLink, PatchEmbeddingTable, RegionEmbeddingTable, SlideEmbeddingTable,
    VerifiedDirectPatchEmbeddingArtifactGraph, VerifiedPatchEmbeddingSupportArtifact,
    VerifiedPatchEmbeddingTableArtifact, VerifiedRegionEmbeddingTableArtifact,
    VerifiedSlideEmbeddingTableArtifact,
};

use super::{
    super::physical::validated_stock_group_range,
    preflight::{
        parquet_failure, prepare_reader, MultiscaleMatrixParquetPreflight,
        PreparedMultiscaleMatrixParquet,
    },
    profile::{schema, COLUMN_COUNT},
};
use crate::columnar::{
    multiscale::{
        enforce_file_budget, enforce_retained_budget, enforce_row_group_budget,
        matrix_decoded_bytes, matrix_dependencies, MatrixPhysicalAccumulator,
        MultiscaleMatrixTable,
    },
    parquet::{profile::ROW_GROUP_ROWS, reader::RowGroupWindow},
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialParquetFailure,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

macro_rules! typed_reader {
    ($bytes:ident, $managed:ident, $table:ty) => {
        #[doc = "Fully decode and validate borrowed canonical C-05 Parquet matrix bytes."]
        pub fn $bytes(
            bytes: &[u8],
            record: &ArtifactRecord,
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<MultiscaleMatrixParquetPreflight, MultiscaleColumnarError> {
            validate_bytes(bytes, record, table, budgets)
        }

        #[doc = "Fully decode one managed C-05 Parquet matrix inside one integrity envelope."]
        pub fn $managed(
            store: &LocalArtifactStore,
            record: &ArtifactRecord,
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<MultiscaleMatrixParquetPreflight, VerifiedReaderError<MultiscaleColumnarError>>
        {
            validate_from_store(store, record, table, budgets)
        }
    };
}

typed_reader!(
    validate_patch_embedding_table_parquet_bytes,
    validate_patch_embedding_table_parquet_from_store,
    PatchEmbeddingTable
);

/// Fully decode borrowed patch-matrix Parquet bytes and mint an exact direct-patch receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_embedding_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    table: &PatchEmbeddingTable,
    source_row_link: &PatchEmbeddingSourceRowLink,
    support: VerifiedPatchEmbeddingSupportArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchEmbeddingTableArtifact, MultiscaleColumnarError> {
    validate_patch_embedding_table_parquet_bytes(bytes, record, table, budgets)?;
    VerifiedPatchEmbeddingTableArtifact::new(record.id(), table, source_row_link, support, graph)
}

/// Fully decode a managed patch-matrix Parquet artifact and mint an exact direct-patch receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_embedding_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    table: &PatchEmbeddingTable,
    source_row_link: &PatchEmbeddingSourceRowLink,
    support: VerifiedPatchEmbeddingSupportArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchEmbeddingTableArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_patch_embedding_table_parquet_from_store(store, record, table, budgets)?;
    VerifiedPatchEmbeddingTableArtifact::new(record.id(), table, source_row_link, support, graph)
        .map_err(VerifiedReaderError::Callback)
}
typed_reader!(
    validate_region_embedding_table_parquet_bytes,
    validate_region_embedding_table_parquet_from_store,
    RegionEmbeddingTable
);

/// Fully decode borrowed derived-region Parquet bytes and mint an exact finalization receipt.
pub fn verify_region_embedding_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    candidate: &DerivedRegionEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedRegionEmbeddingTableArtifact, MultiscaleColumnarError> {
    validate_region_embedding_table_parquet_bytes(bytes, record, candidate.table(), budgets)?;
    VerifiedRegionEmbeddingTableArtifact::new(record.id(), candidate)
}

/// Fully decode a managed derived-region Parquet artifact and mint an exact finalization receipt.
pub fn verify_region_embedding_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    candidate: &DerivedRegionEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedRegionEmbeddingTableArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_region_embedding_table_parquet_from_store(store, record, candidate.table(), budgets)?;
    VerifiedRegionEmbeddingTableArtifact::new(record.id(), candidate)
        .map_err(VerifiedReaderError::Callback)
}
typed_reader!(
    validate_slide_embedding_table_parquet_bytes,
    validate_slide_embedding_table_parquet_from_store,
    SlideEmbeddingTable
);

/// Fully decode borrowed derived-slide Parquet bytes and mint an exact finalization receipt.
pub fn verify_slide_embedding_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    candidate: &DerivedSlideEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedSlideEmbeddingTableArtifact, MultiscaleColumnarError> {
    validate_slide_embedding_table_parquet_bytes(bytes, record, candidate.table(), budgets)?;
    VerifiedSlideEmbeddingTableArtifact::new(record.id(), candidate)
}

/// Fully decode a managed derived-slide Parquet artifact and mint an exact finalization receipt.
pub fn verify_slide_embedding_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    candidate: &DerivedSlideEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedSlideEmbeddingTableArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_slide_embedding_table_parquet_from_store(store, record, candidate.table(), budgets)?;
    VerifiedSlideEmbeddingTableArtifact::new(record.id(), candidate)
        .map_err(VerifiedReaderError::Callback)
}

fn validate_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    table: &dyn MultiscaleMatrixTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<MultiscaleMatrixParquetPreflight, MultiscaleColumnarError> {
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, table)?;
    let mut source = Cursor::new(bytes);
    let prepared = prepare_reader(
        &mut source,
        encoded_len,
        marklab_project::ContentDigest::from_bytes(bytes),
        table,
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    let summary = MultiscaleMatrixParquetPreflight::from_prepared(&prepared);
    decode_parquet(&mut source, table, prepared, budgets)?;
    Ok(summary)
}

fn validate_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    table: &dyn MultiscaleMatrixTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<MultiscaleMatrixParquetPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        let encoded_len = reader
            .seek(SeekFrom::End(0))
            .map_err(|_| parquet_failure(SpatialParquetFailure::ArtifactRead))?;
        enforce_file_budget(encoded_len, budgets)?;
        validate_record(record, encoded_len, table)?;
        let prepared = prepare_reader(
            reader,
            encoded_len,
            record.content().digest(),
            table,
            budgets,
        )?;
        validate_identity(&prepared, record)?;
        let summary = MultiscaleMatrixParquetPreflight::from_prepared(&prepared);
        decode_parquet(reader, table, prepared, budgets)?;
        Ok(summary)
    })
}

fn decode_parquet<R: Read + Seek + ?Sized>(
    source: &mut R,
    table: &dyn MultiscaleMatrixTable,
    prepared: PreparedMultiscaleMatrixParquet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    let maximum_group_peak = maximum_group_peak(&prepared.metadata, table)?;
    let retained = prepared
        .metadata
        .memory_size()
        .checked_mul(2)
        .and_then(|value| value.checked_add(maximum_group_peak))
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?
        .max(prepared.retained_preflight_bytes);
    enforce_retained_budget(retained, budgets)?;

    let metadata = Arc::new(prepared.metadata);
    let expected_schema = schema(table)?;
    let reader_metadata = ArrowReaderMetadata::try_new(
        Arc::clone(&metadata),
        ArrowReaderOptions::new().with_schema(Arc::new(expected_schema.clone())),
    )
    .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
    let mut accumulator = MatrixPhysicalAccumulator::new(table)?;
    let mut global_row = 0_usize;
    for (group_index, group) in metadata.row_groups().iter().enumerate() {
        let rows = usize::try_from(group.num_rows())
            .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
        let (start, length) = validated_stock_group_range(group, COLUMN_COUNT)?;
        let peak = estimate_group_peak(group, table, global_row)?;
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
        if reader.schema().as_ref() != &expected_schema {
            return Err(parquet_failure(SpatialParquetFailure::StockDecode));
        }
        let group_start = global_row;
        for decoded in &mut reader {
            let batch = decoded.map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
            validate_batch(&batch, table, &mut accumulator, &mut global_row)?;
        }
        if global_row
            .checked_sub(group_start)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?
            != rows
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
        }
    }
    if global_row != table.row_count()
        || accumulator.finish()? != prepared.qc_summary
        || prepared.qc_summary != table.qc_summary()
    {
        return Err(parquet_failure(SpatialParquetFailure::InvalidCanonicalRows));
    }
    Ok(())
}

fn maximum_group_peak(
    metadata: &parquet::file::metadata::ParquetMetaData,
    table: &dyn MultiscaleMatrixTable,
) -> Result<usize, MultiscaleColumnarError> {
    let mut maximum = 0_usize;
    let mut group_start = 0_usize;
    for group in metadata.row_groups() {
        maximum = maximum.max(estimate_group_peak(group, table, group_start)?);
        group_start = group_start
            .checked_add(
                usize::try_from(group.num_rows())
                    .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?,
            )
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    if group_start != table.row_count() {
        return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
    }
    Ok(maximum)
}

fn validate_batch(
    batch: &arrow::record_batch::RecordBatch,
    table: &dyn MultiscaleMatrixTable,
    accumulator: &mut MatrixPhysicalAccumulator,
    global_row: &mut usize,
) -> Result<(), MultiscaleColumnarError> {
    if batch.num_columns() != COLUMN_COUNT {
        return Err(parquet_failure(SpatialParquetFailure::StockDecode));
    }
    for column in batch.columns() {
        column
            .to_data()
            .validate_full()
            .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
    }
    let ids = string_column(batch, 0)?;
    let embeddings = batch
        .column(1)
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::StockDecode))?;
    let values = embeddings
        .values()
        .as_any()
        .downcast_ref::<Float32Array>()
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::StockDecode))?;
    let statuses = string_column(batch, 2)?;
    if ids.null_count() != 0
        || embeddings.null_count() != 0
        || values.null_count() != 0
        || statuses.null_count() != 0
        || embeddings.value_length()
            != i32::try_from(table.dimension())
                .map_err(|_| MultiscaleColumnarError::SizeOverflow)?
    {
        return Err(parquet_failure(SpatialParquetFailure::StockDecode));
    }
    let dimension =
        usize::try_from(table.dimension()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    for local in 0..batch.num_rows() {
        let expected = table
            .row(*global_row)
            .map_err(|_| parquet_failure(SpatialParquetFailure::InvalidRowCount))?;
        let value_start = local
            .checked_mul(dimension)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let value_end = value_start
            .checked_add(dimension)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        let vector = values
            .values()
            .get(value_start..value_end)
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidCanonicalRows))?;
        let status = parse_embedding_status(statuses.value(local))?;
        if ids.value(local) != expected.id()
            || status != expected.status()
            || vector.iter().copied().enumerate().any(|(column, value)| {
                value.to_bits()
                    != expected
                        .vector()
                        .map_or(0.0, |expected_vector| expected_vector[column])
                        .to_bits()
            })
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidCanonicalRows));
        }
        accumulator.push(
            expected.id(),
            status,
            (status == EmbeddingStatus::Present).then_some(vector),
        )?;
        *global_row = global_row
            .checked_add(1)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn estimate_group_peak(
    group: &RowGroupMetaData,
    table: &dyn MultiscaleMatrixTable,
    group_start: usize,
) -> Result<usize, MultiscaleColumnarError> {
    let (_, encoded) = validated_stock_group_range(group, COLUMN_COUNT)?;
    let rows = usize::try_from(group.num_rows())
        .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
    let group_end = group_start
        .checked_add(rows)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let decoded = matrix_decoded_bytes(table, group_start, group_end)?;
    encoded
        .checked_mul(2)
        .and_then(|value| value.checked_add(decoded.checked_mul(2)?))
        .and_then(|value| value.checked_add(rows.checked_mul(4)?))
        .ok_or(MultiscaleColumnarError::SizeOverflow)
}

fn validate_record(
    record: &ArtifactRecord,
    encoded_len: u64,
    table: &dyn MultiscaleMatrixTable,
) -> Result<(), MultiscaleColumnarError> {
    let profile = table.profile();
    let row_count =
        u64::try_from(table.row_count()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    if !profile.record_matches_encoding(
        record,
        SpatialPhysicalEncoding::Parquet,
        row_count,
        table.dimension(),
    ) || record.content().byte_len() != encoded_len
        || record.dependencies() != matrix_dependencies(table)?
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    prepared: &PreparedMultiscaleMatrixParquet,
    record: &ArtifactRecord,
) -> Result<(), MultiscaleColumnarError> {
    if prepared.encoded_byte_len != record.content().byte_len()
        || prepared.content_digest != record.content().digest()
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
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

fn parse_embedding_status(value: &str) -> Result<EmbeddingStatus, MultiscaleColumnarError> {
    match value {
        "present" => Ok(EmbeddingStatus::Present),
        "missing_vector" => Ok(EmbeddingStatus::MissingVector),
        "extraction_failed" => Ok(EmbeddingStatus::ExtractionFailed),
        "qc_rejected" => Ok(EmbeddingStatus::QcRejected),
        _ => Err(parquet_failure(SpatialParquetFailure::InvalidCanonicalRows)),
    }
}

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
    EmbeddingColumnarBudgets, PatchRegionLink, VerifiedPatchRegionInputArtifactGraph,
    VerifiedPatchRegionLinkArtifact,
};

use super::{
    super::physical::validated_stock_group_range,
    preflight::{
        parquet_failure, prepare_reader, PatchRegionParquetPreflight, PreparedPatchRegionParquet,
    },
    profile::{dependencies, schema, COLUMN_COUNT},
};
use crate::columnar::{
    multiscale::{
        enforce_file_budget, enforce_retained_budget, enforce_row_group_budget,
        patch_region_decoded_bytes,
    },
    parquet::{profile::ROW_GROUP_ROWS, reader::RowGroupWindow},
    MultiscaleColumnarError, SpatialParquetFailure,
};
use crate::multiscale::physical::{
    record_matches_encoding, SpatialArtifactRole, SpatialPhysicalEncoding,
};

/// Fully decode and validate borrowed canonical Parquet patch-region-link bytes.
pub fn validate_patch_region_link_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchRegionParquetPreflight, MultiscaleColumnarError> {
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, link)?;
    let mut source = Cursor::new(bytes);
    let prepared = prepare_reader(
        &mut source,
        encoded_len,
        marklab_project::ContentDigest::from_bytes(bytes),
        link,
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    let summary = PatchRegionParquetPreflight::from_prepared(&prepared);
    decode_parquet(&mut source, link, prepared, budgets)?;
    Ok(summary)
}

/// Fully decode one managed Parquet patch-region link in one integrity envelope.
pub fn validate_patch_region_link_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchRegionParquetPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        let encoded_len = reader
            .seek(SeekFrom::End(0))
            .map_err(|_| parquet_failure(SpatialParquetFailure::ArtifactRead))?;
        enforce_file_budget(encoded_len, budgets)?;
        validate_record(record, encoded_len, link)?;
        let prepared = prepare_reader(
            reader,
            encoded_len,
            record.content().digest(),
            link,
            budgets,
        )?;
        validate_identity(&prepared, record)?;
        let summary = PatchRegionParquetPreflight::from_prepared(&prepared);
        decode_parquet(reader, link, prepared, budgets)?;
        Ok(summary)
    })
}

/// Verify borrowed Parquet bytes and mint a graph-bound patch-region-link receipt.
pub fn verify_patch_region_link_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &PatchRegionLink,
    graph: VerifiedPatchRegionInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchRegionLinkArtifact, MultiscaleColumnarError> {
    validate_patch_region_link_parquet_bytes(bytes, record, link, budgets)?;
    VerifiedPatchRegionLinkArtifact::new(record.id(), link, graph)
}

/// Verify one managed Parquet artifact and mint a graph-bound patch-region-link receipt.
pub fn verify_patch_region_link_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &PatchRegionLink,
    graph: VerifiedPatchRegionInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchRegionLinkArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_patch_region_link_parquet_from_store(store, record, link, budgets)?;
    VerifiedPatchRegionLinkArtifact::new(record.id(), link, graph)
        .map_err(VerifiedReaderError::Callback)
}

fn decode_parquet<R: Read + Seek + ?Sized>(
    source: &mut R,
    link: &PatchRegionLink,
    prepared: PreparedPatchRegionParquet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    let maximum_group_peak = maximum_group_peak(&prepared.metadata, link)?;
    let retained = prepared
        .metadata
        .memory_size()
        .checked_add(maximum_group_peak)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?
        .max(prepared.retained_preflight_bytes);
    enforce_retained_budget(retained, budgets)?;

    let metadata = Arc::new(prepared.metadata);
    let schema = schema();
    let reader_metadata = ArrowReaderMetadata::try_new(
        Arc::clone(&metadata),
        ArrowReaderOptions::new().with_schema(Arc::new(schema.clone())),
    )
    .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
    let mut global_row = 0_usize;
    for (group_index, group) in metadata.row_groups().iter().enumerate() {
        let rows = usize::try_from(group.num_rows())
            .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
        let (start, length) = validated_stock_group_range(group, COLUMN_COUNT)?;
        let peak = estimate_group_peak(group, link, global_row)?;
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
            validate_batch(&batch, link, &mut global_row)?;
        }
        if global_row
            .checked_sub(group_start)
            .ok_or(MultiscaleColumnarError::SizeOverflow)?
            != rows
        {
            return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
        }
    }
    if global_row != link.nonzero_relation_count() {
        return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
    }
    Ok(())
}

fn maximum_group_peak(
    metadata: &parquet::file::metadata::ParquetMetaData,
    link: &PatchRegionLink,
) -> Result<usize, MultiscaleColumnarError> {
    let mut maximum = 0_usize;
    let mut group_start = 0_usize;
    for group in metadata.row_groups() {
        maximum = maximum.max(estimate_group_peak(group, link, group_start)?);
        group_start = group_start
            .checked_add(
                usize::try_from(group.num_rows())
                    .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?,
            )
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    }
    if group_start != link.nonzero_relation_count() {
        return Err(parquet_failure(SpatialParquetFailure::InvalidRowCount));
    }
    Ok(maximum)
}

fn validate_batch(
    batch: &arrow::record_batch::RecordBatch,
    link: &PatchRegionLink,
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
    let patches = string_column(batch, 0)?;
    let regions = string_column(batch, 1)?;
    let relations = string_column(batch, 2)?;
    let numerators = u64_column(batch, 3)?;
    let denominators = u64_column(batch, 4)?;
    if [
        patches.null_count(),
        regions.null_count(),
        relations.null_count(),
        numerators.null_count(),
        denominators.null_count(),
    ]
    .iter()
    .any(|count| *count != 0)
    {
        return Err(parquet_failure(SpatialParquetFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        let row = link
            .nonzero_relations()
            .get(*global_row)
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowCount))?;
        if patches.value(local) != row.patch_id().as_str()
            || regions.value(local) != row.region_id().as_str()
            || relations.value(local) != row.relation().wire_name()
            || numerators.value(local) != row.numerator()
            || denominators.value(local) != row.denominator()
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
    link: &PatchRegionLink,
    group_start: usize,
) -> Result<usize, MultiscaleColumnarError> {
    let (_, encoded) = validated_stock_group_range(group, COLUMN_COUNT)?;
    let rows = usize::try_from(group.num_rows())
        .map_err(|_| parquet_failure(SpatialParquetFailure::StockDecode))?;
    let group_end = group_start
        .checked_add(rows)
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    let decoded = patch_region_decoded_bytes(
        link.nonzero_relations()
            .get(group_start..group_end)
            .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidRowCount))?,
    )?;
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
    link: &PatchRegionLink,
) -> Result<(), MultiscaleColumnarError> {
    let row_count = u64::try_from(link.nonzero_relation_count())
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    if !record_matches_encoding(
        record,
        SpatialArtifactRole::PatchRegion,
        SpatialPhysicalEncoding::Parquet,
        row_count,
    ) || record.content().byte_len() != encoded_len
        || record.dependencies() != dependencies(link)
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    prepared: &PreparedPatchRegionParquet,
    record: &ArtifactRecord,
) -> Result<(), MultiscaleColumnarError> {
    if prepared.encoded_byte_len != record.content().byte_len()
        || prepared.content_digest != record.content().digest()
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

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
    ColumnarError, ParquetFailure,
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
) -> Result<PatchRegionParquetPreflight, ColumnarError> {
    let encoded_len = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
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
) -> Result<PatchRegionParquetPreflight, VerifiedReaderError<ColumnarError>> {
    store.with_verified_reader(record, |reader| {
        let encoded_len = reader
            .seek(SeekFrom::End(0))
            .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
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
) -> Result<VerifiedPatchRegionLinkArtifact, ColumnarError> {
    let context = crate::PatchRegionVerifyContext { link, graph };
    crate::columnar::verify::<crate::Parquet, crate::PatchRegionEntity>(
        Cursor::new(bytes),
        record,
        &context,
        budgets,
    )
}

/// Verify one managed Parquet artifact and mint a graph-bound patch-region-link receipt.
pub fn verify_patch_region_link_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &PatchRegionLink,
    graph: VerifiedPatchRegionInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchRegionLinkArtifact, VerifiedReaderError<ColumnarError>> {
    let context = crate::PatchRegionVerifyContext { link, graph };
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.verify::<crate::Parquet, crate::PatchRegionEntity>(&context)
    })
}

fn decode_parquet<R: Read + Seek + ?Sized>(
    source: &mut R,
    link: &PatchRegionLink,
    prepared: PreparedPatchRegionParquet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), ColumnarError> {
    let maximum_group_peak = maximum_group_peak(&prepared.metadata, link)?;
    let retained = prepared
        .metadata
        .memory_size()
        .checked_add(maximum_group_peak)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(ColumnarError::SizeOverflow)?
        .max(prepared.retained_preflight_bytes);
    enforce_retained_budget(retained, budgets)?;

    let metadata = Arc::new(prepared.metadata);
    let schema = schema();
    let reader_metadata = ArrowReaderMetadata::try_new(
        Arc::clone(&metadata),
        ArrowReaderOptions::new().with_schema(Arc::new(schema.clone())),
    )
    .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let mut global_row = 0_usize;
    for (group_index, group) in metadata.row_groups().iter().enumerate() {
        let rows = usize::try_from(group.num_rows())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let (start, length) = validated_stock_group_range(group, COLUMN_COUNT)?;
        let peak = estimate_group_peak(group, link, global_row)?;
        enforce_row_group_budget(peak, budgets)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ColumnarError::AllocationFailed { requested: length })?;
        bytes.resize(length, 0);
        source
            .seek(SeekFrom::Start(start))
            .and_then(|_| source.read_exact(&mut bytes))
            .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
        let end = start
            .checked_add(u64::try_from(length).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)?;
        let window = RowGroupWindow::new(start, end, Bytes::from(bytes));
        let mut reader =
            ParquetRecordBatchReaderBuilder::new_with_metadata(window, reader_metadata.clone())
                .with_row_groups(vec![group_index])
                .with_batch_size(ROW_GROUP_ROWS)
                .build()
                .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        if reader.schema().as_ref() != &schema {
            return Err(parquet_failure(ParquetFailure::StockDecode));
        }
        let group_start = global_row;
        for decoded in &mut reader {
            let batch = decoded.map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
            validate_batch(&batch, link, &mut global_row)?;
        }
        if global_row
            .checked_sub(group_start)
            .ok_or(ColumnarError::SizeOverflow)?
            != rows
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowCount));
        }
    }
    if global_row != link.nonzero_relation_count() {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(())
}

fn maximum_group_peak(
    metadata: &parquet::file::metadata::ParquetMetaData,
    link: &PatchRegionLink,
) -> Result<usize, ColumnarError> {
    let mut maximum = 0_usize;
    let mut group_start = 0_usize;
    for group in metadata.row_groups() {
        maximum = maximum.max(estimate_group_peak(group, link, group_start)?);
        group_start = group_start
            .checked_add(
                usize::try_from(group.num_rows())
                    .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?,
            )
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    if group_start != link.nonzero_relation_count() {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(maximum)
}

fn validate_batch(
    batch: &arrow::record_batch::RecordBatch,
    link: &PatchRegionLink,
    global_row: &mut usize,
) -> Result<(), ColumnarError> {
    visit_batch(
        batch,
        global_row,
        |index, patch, region, relation, numerator, denominator| {
            let row = link
                .nonzero_relations()
                .get(index)
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowCount))?;
            if patch != row.patch_id().as_str()
                || region != row.region_id().as_str()
                || relation != row.relation()
                || numerator != row.numerator()
                || denominator != row.denominator()
            {
                return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
            }
            Ok(())
        },
    )
}

pub(in crate::columnar::parquet) fn visit_batch(
    batch: &arrow::record_batch::RecordBatch,
    global_row: &mut usize,
    mut visit: impl FnMut(
        usize,
        &str,
        &str,
        crate::PatchRegionRelation,
        u64,
        u64,
    ) -> Result<(), ColumnarError>,
) -> Result<(), ColumnarError> {
    if batch.num_columns() != COLUMN_COUNT {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for column in batch.columns() {
        column
            .to_data()
            .validate_full()
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
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
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        let numerator = numerators.value(local);
        let denominator = denominators.value(local);
        let relation = match relations.value(local) {
            "fully_contained" if numerator == 1 && denominator == 1 => {
                crate::PatchRegionRelation::FullyContained
            }
            "partial_overlap"
                if numerator > 0
                    && numerator < denominator
                    && crate::rational::greatest_common_divisor(numerator, denominator) == 1 =>
            {
                crate::PatchRegionRelation::PartialOverlap
            }
            _ => return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows)),
        };
        visit(
            *global_row,
            patches.value(local),
            regions.value(local),
            relation,
            numerator,
            denominator,
        )?;
        *global_row = global_row
            .checked_add(1)
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn estimate_group_peak(
    group: &RowGroupMetaData,
    link: &PatchRegionLink,
    group_start: usize,
) -> Result<usize, ColumnarError> {
    let (_, encoded) = validated_stock_group_range(group, COLUMN_COUNT)?;
    let rows = usize::try_from(group.num_rows())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let group_end = group_start
        .checked_add(rows)
        .ok_or(ColumnarError::SizeOverflow)?;
    let decoded = patch_region_decoded_bytes(
        link.nonzero_relations()
            .get(group_start..group_end)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowCount))?,
    )?;
    encoded
        .checked_mul(2)
        .and_then(|value| value.checked_add(decoded.checked_mul(2)?))
        .and_then(|value| value.checked_add(rows.checked_mul(4)?))
        .ok_or(ColumnarError::SizeOverflow)
}

fn string_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&StringArray, ColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))
}

fn u64_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&UInt64Array, ColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))
}

pub(in crate::columnar::parquet) fn validate_record(
    record: &ArtifactRecord,
    encoded_len: u64,
    link: &PatchRegionLink,
) -> Result<(), ColumnarError> {
    let row_count =
        u64::try_from(link.nonzero_relation_count()).map_err(|_| ColumnarError::SizeOverflow)?;
    if !record_matches_encoding(
        record,
        SpatialArtifactRole::PatchRegion,
        SpatialPhysicalEncoding::Parquet,
        row_count,
    ) || record.content().byte_len() != encoded_len
        || record.dependencies() != dependencies(link)
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    prepared: &PreparedPatchRegionParquet,
    record: &ArtifactRecord,
) -> Result<(), ColumnarError> {
    if prepared.encoded_byte_len != record.content().byte_len()
        || prepared.content_digest != record.content().digest()
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

pub(crate) fn verify_patch_region_link_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &crate::PatchRegionVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchRegionLinkArtifact, ColumnarError> {
    enforce_file_budget(record.content().byte_len(), budgets)?;
    validate_record(record, record.content().byte_len(), context.link)?;
    let prepared = prepare_reader(
        reader,
        record.content().byte_len(),
        record.content().digest(),
        context.link,
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    decode_parquet(reader, context.link, prepared, budgets)?;
    VerifiedPatchRegionLinkArtifact::new(record.id(), context.link, context.graph)
}

use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    sync::Arc,
};

use arrow::array::{Array, Int64Array, RecordBatchReader, StringArray};
use bytes::Bytes;
use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};
use parquet::{
    arrow::arrow_reader::{
        ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
    },
    file::metadata::RowGroupMetaData,
};

use crate::{
    ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph,
    VerifiedDirectPatchEmbeddingArtifactGraph, VerifiedPatchFootprintArtifact,
    VerifiedPatchOverlapArtifact,
};

use super::{
    super::physical::validated_stock_group_range,
    preflight::{
        parquet_failure, prepare_reader, CommonParquetPreflight, PatchFootprintParquetPreflight,
        PatchOverlapParquetPreflight,
    },
    profile::{expected_schema, SpatialParquetProfile},
};
use crate::{
    columnar::{
        multiscale::{enforce_file_budget, enforce_retained_budget, enforce_row_group_budget},
        parquet::{profile::ROW_GROUP_ROWS, reader::RowGroupWindow},
        ColumnarError, EmbeddingColumnarBudgets, ParquetFailure,
    },
    multiscale::physical::{record_matches_encoding, SpatialArtifactRole, SpatialPhysicalEncoding},
};

/// Fully decode and validate borrowed canonical footprint Parquet bytes.
pub fn validate_patch_footprint_set_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchFootprintParquetPreflight, ColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    validate_record(
        record,
        encoded,
        SpatialArtifactRole::Footprint,
        footprints.row_count(),
        footprint_dependencies(footprints),
    )?;
    let prepared = prepare_reader(
        &mut Cursor::new(bytes),
        encoded,
        marklab_project::ContentDigest::from_bytes(bytes),
        SpatialParquetProfile::Footprint,
        expected,
        context,
        footprints,
        None,
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    let summary = PatchFootprintParquetPreflight::from_common(&prepared);
    decode_parquet(
        &mut Cursor::new(bytes),
        SpatialParquetProfile::Footprint,
        footprints,
        None,
        prepared,
        budgets,
    )?;
    Ok(summary)
}

/// Fully decode and validate borrowed canonical overlap Parquet bytes.
pub fn validate_patch_overlap_graph_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchOverlapParquetPreflight, ColumnarError> {
    let encoded = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded, budgets)?;
    validate_record(
        record,
        encoded,
        SpatialArtifactRole::Overlap,
        overlap.edge_count(),
        overlap_dependencies(footprints, overlap),
    )?;
    let prepared = prepare_reader(
        &mut Cursor::new(bytes),
        encoded,
        marklab_project::ContentDigest::from_bytes(bytes),
        SpatialParquetProfile::Overlap,
        expected,
        context,
        footprints,
        Some(overlap),
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    let summary = PatchOverlapParquetPreflight::from_common(&prepared);
    decode_parquet(
        &mut Cursor::new(bytes),
        SpatialParquetProfile::Overlap,
        footprints,
        Some(overlap),
        prepared,
        budgets,
    )?;
    Ok(summary)
}

/// Fully decode a managed footprint Parquet artifact inside one integrity envelope.
pub fn validate_patch_footprint_set_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchFootprintParquetPreflight, VerifiedReaderError<ColumnarError>> {
    store.with_verified_reader(record, |reader| {
        let encoded = reader
            .seek(SeekFrom::End(0))
            .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
        enforce_file_budget(encoded, budgets)?;
        validate_record(
            record,
            encoded,
            SpatialArtifactRole::Footprint,
            footprints.row_count(),
            footprint_dependencies(footprints),
        )?;
        let prepared = prepare_reader(
            reader,
            encoded,
            record.content().digest(),
            SpatialParquetProfile::Footprint,
            expected,
            context,
            footprints,
            None,
            budgets,
        )?;
        validate_identity(&prepared, record)?;
        let summary = PatchFootprintParquetPreflight::from_common(&prepared);
        decode_parquet(
            reader,
            SpatialParquetProfile::Footprint,
            footprints,
            None,
            prepared,
            budgets,
        )?;
        Ok(summary)
    })
}

/// Fully decode a managed overlap Parquet artifact inside one integrity envelope.
pub fn validate_patch_overlap_graph_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchOverlapParquetPreflight, VerifiedReaderError<ColumnarError>> {
    store.with_verified_reader(record, |reader| {
        let encoded = reader
            .seek(SeekFrom::End(0))
            .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
        enforce_file_budget(encoded, budgets)?;
        validate_record(
            record,
            encoded,
            SpatialArtifactRole::Overlap,
            overlap.edge_count(),
            overlap_dependencies(footprints, overlap),
        )?;
        let prepared = prepare_reader(
            reader,
            encoded,
            record.content().digest(),
            SpatialParquetProfile::Overlap,
            expected,
            context,
            footprints,
            Some(overlap),
            budgets,
        )?;
        validate_identity(&prepared, record)?;
        let summary = PatchOverlapParquetPreflight::from_common(&prepared);
        decode_parquet(
            reader,
            SpatialParquetProfile::Overlap,
            footprints,
            Some(overlap),
            prepared,
            budgets,
        )?;
        Ok(summary)
    })
}

/// Verify borrowed footprint Parquet bytes and mint a graph-bound runtime receipt.
pub fn verify_patch_footprint_set_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchFootprintArtifact, ColumnarError> {
    let context = crate::PatchFootprintVerifyContext {
        values: crate::PatchFootprintWriteContext {
            expected,
            context,
            footprints,
        },
        graph,
    };
    crate::columnar::verify::<crate::Parquet, crate::PatchFootprintEntity>(
        Cursor::new(bytes),
        record,
        &context,
        budgets,
    )
}

/// Verify a managed footprint Parquet artifact and mint a graph-bound runtime receipt.
pub fn verify_patch_footprint_set_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchFootprintArtifact, VerifiedReaderError<ColumnarError>> {
    let context = crate::PatchFootprintVerifyContext {
        values: crate::PatchFootprintWriteContext {
            expected,
            context,
            footprints,
        },
        graph,
    };
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.verify::<crate::Parquet, crate::PatchFootprintEntity>(&context)
    })
}

/// Verify borrowed overlap Parquet bytes and mint a footprint- and graph-bound receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_overlap_graph_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    verified_footprints: VerifiedPatchFootprintArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchOverlapArtifact, ColumnarError> {
    let context = crate::PatchOverlapVerifyContext {
        values: crate::PatchOverlapWriteContext {
            expected,
            context,
            footprints,
            overlap,
        },
        verified_footprints,
        graph,
    };
    crate::columnar::verify::<crate::Parquet, crate::PatchOverlapEntity>(
        Cursor::new(bytes),
        record,
        &context,
        budgets,
    )
}

/// Verify a managed overlap Parquet artifact and mint a footprint- and graph-bound receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_overlap_graph_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    verified_footprints: VerifiedPatchFootprintArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchOverlapArtifact, VerifiedReaderError<ColumnarError>> {
    let context = crate::PatchOverlapVerifyContext {
        values: crate::PatchOverlapWriteContext {
            expected,
            context,
            footprints,
            overlap,
        },
        verified_footprints,
        graph,
    };
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.verify::<crate::Parquet, crate::PatchOverlapEntity>(&context)
    })
}

fn decode_parquet<R: Read + Seek + ?Sized>(
    source: &mut R,
    profile: SpatialParquetProfile,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    prepared: CommonParquetPreflight,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), ColumnarError> {
    let maximum_group_peak = prepared
        .metadata
        .row_groups()
        .iter()
        .try_fold(0_usize, |maximum, group| {
            estimate_group_peak(group).map(|required| maximum.max(required))
        })?;
    let retained = prepared
        .metadata
        .memory_size()
        .checked_add(maximum_group_peak)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(ColumnarError::SizeOverflow)?
        .max(prepared.retained_preflight_bytes);
    enforce_retained_budget(retained, budgets)?;

    let metadata = Arc::new(prepared.metadata);
    let schema = expected_schema(profile);
    let reader_metadata = ArrowReaderMetadata::try_new(
        Arc::clone(&metadata),
        ArrowReaderOptions::new().with_schema(Arc::new(schema.clone())),
    )
    .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let mut global_row = 0_usize;
    for (group_index, group) in metadata.row_groups().iter().enumerate() {
        let rows = usize::try_from(group.num_rows())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let (start, length) = validated_stock_group_range(group, profile.column_count())?;
        let peak = estimate_group_peak(group)?;
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
            validate_batch(&batch, profile, footprints, overlap, &mut global_row)?;
        }
        if global_row
            .checked_sub(group_start)
            .ok_or(ColumnarError::SizeOverflow)?
            != rows
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowCount));
        }
    }
    let expected_rows = match profile {
        SpatialParquetProfile::Footprint => footprints.row_count(),
        SpatialParquetProfile::Overlap => overlap
            .ok_or(ColumnarError::ArtifactBindingMismatch)?
            .edge_count(),
    };
    if global_row != expected_rows {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(())
}

fn validate_batch(
    batch: &arrow::record_batch::RecordBatch,
    profile: SpatialParquetProfile,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    global_row: &mut usize,
) -> Result<(), ColumnarError> {
    match profile {
        SpatialParquetProfile::Footprint => {
            visit_footprint_batch(batch, global_row, |index, id, origin| {
                let row = footprints
                    .footprints()
                    .get(index)
                    .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowCount))?;
                if id != row.patch_id().as_str() || origin != row.origin_px() {
                    return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
                }
                Ok(())
            })
        }
        SpatialParquetProfile::Overlap => {
            let overlap = overlap.ok_or(ColumnarError::ArtifactBindingMismatch)?;
            visit_overlap_batch(batch, global_row, |index, left, right| {
                let row = overlap
                    .edges()
                    .get(index)
                    .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowCount))?;
                if left != row.left_patch_id().as_str() || right != row.right_patch_id().as_str() {
                    return Err(parquet_failure(ParquetFailure::InvalidCanonicalRows));
                }
                Ok(())
            })
        }
    }
}

pub(in crate::columnar::parquet) fn visit_footprint_batch(
    batch: &arrow::record_batch::RecordBatch,
    global_row: &mut usize,
    mut visit: impl FnMut(usize, &str, [i64; 2]) -> Result<(), ColumnarError>,
) -> Result<(), ColumnarError> {
    super::super::read::validate_batch_arrays(batch, 3)?;
    let ids = string_column(batch, 0)?;
    let x = i64_column(batch, 1)?;
    let y = i64_column(batch, 2)?;
    if ids.null_count() != 0 || x.null_count() != 0 || y.null_count() != 0 {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        visit(
            *global_row,
            ids.value(local),
            [x.value(local), y.value(local)],
        )?;
        *global_row = global_row
            .checked_add(1)
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn visit_overlap_batch(
    batch: &arrow::record_batch::RecordBatch,
    global_row: &mut usize,
    mut visit: impl FnMut(usize, &str, &str) -> Result<(), ColumnarError>,
) -> Result<(), ColumnarError> {
    super::super::read::validate_batch_arrays(batch, 2)?;
    let left = string_column(batch, 0)?;
    let right = string_column(batch, 1)?;
    if left.null_count() != 0 || right.null_count() != 0 {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for local in 0..batch.num_rows() {
        visit(*global_row, left.value(local), right.value(local))?;
        *global_row = global_row
            .checked_add(1)
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    Ok(())
}

fn estimate_group_peak(group: &RowGroupMetaData) -> Result<usize, ColumnarError> {
    let (_, encoded) = validated_stock_group_range(group, group.num_columns())?;
    let rows = usize::try_from(group.num_rows())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let arrow_output = encoded
        .checked_add(rows.checked_mul(24).ok_or(ColumnarError::SizeOverflow)?)
        .and_then(|value| value.checked_add((rows + 1).checked_mul(8)?))
        .ok_or(ColumnarError::SizeOverflow)?;
    encoded
        .checked_mul(2)
        .and_then(|value| value.checked_add(arrow_output.checked_mul(2)?))
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

fn i64_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&Int64Array, ColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))
}

pub(in crate::columnar::parquet) fn validate_record(
    record: &ArtifactRecord,
    encoded_len: u64,
    role: SpatialArtifactRole,
    rows: usize,
    dependencies: Vec<marklab_project::ArtifactId>,
) -> Result<(), ColumnarError> {
    if record.content().byte_len() != encoded_len
        || record.dependencies() != dependencies
        || !record_matches_encoding(
            record,
            role,
            SpatialPhysicalEncoding::Parquet,
            u64::try_from(rows).map_err(|_| ColumnarError::SizeOverflow)?,
        )
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    prepared: &CommonParquetPreflight,
    record: &ArtifactRecord,
) -> Result<(), ColumnarError> {
    if prepared.content_digest != record.content().digest()
        || prepared.encoded_byte_len != record.content().byte_len()
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

pub(in crate::columnar::parquet) fn footprint_dependencies(
    footprints: &PatchFootprintSet,
) -> Vec<marklab_project::ArtifactId> {
    let mut dependencies = vec![
        footprints.expected_patches_artifact_id(),
        footprints.patch_context_artifact_id(),
    ];
    dependencies.sort_unstable();
    dependencies
}

pub(in crate::columnar::parquet) fn overlap_dependencies(
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
) -> Vec<marklab_project::ArtifactId> {
    let mut dependencies = vec![
        overlap.expected_patches_artifact_id(),
        footprints.patch_context_artifact_id(),
        overlap.patch_footprints_artifact_id(),
    ];
    dependencies.sort_unstable();
    dependencies
}

pub(crate) fn verify_patch_footprint_set_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &crate::PatchFootprintVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchFootprintArtifact, ColumnarError> {
    let values = &context.values;
    validate_record(
        record,
        record.content().byte_len(),
        SpatialArtifactRole::Footprint,
        values.footprints.row_count(),
        footprint_dependencies(values.footprints),
    )?;
    let prepared = prepare_reader(
        reader,
        record.content().byte_len(),
        record.content().digest(),
        SpatialParquetProfile::Footprint,
        values.expected,
        values.context,
        values.footprints,
        None,
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    decode_parquet(
        reader,
        SpatialParquetProfile::Footprint,
        values.footprints,
        None,
        prepared,
        budgets,
    )?;
    VerifiedPatchFootprintArtifact::new(
        record.id(),
        values.context,
        values.footprints,
        context.graph,
    )
}

pub(crate) fn verify_patch_overlap_graph_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &crate::PatchOverlapVerifyContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchOverlapArtifact, ColumnarError> {
    let values = &context.values;
    validate_record(
        record,
        record.content().byte_len(),
        SpatialArtifactRole::Overlap,
        values.overlap.edge_count(),
        overlap_dependencies(values.footprints, values.overlap),
    )?;
    let prepared = prepare_reader(
        reader,
        record.content().byte_len(),
        record.content().digest(),
        SpatialParquetProfile::Overlap,
        values.expected,
        values.context,
        values.footprints,
        Some(values.overlap),
        budgets,
    )?;
    validate_identity(&prepared, record)?;
    decode_parquet(
        reader,
        SpatialParquetProfile::Overlap,
        values.footprints,
        Some(values.overlap),
        prepared,
        budgets,
    )?;
    VerifiedPatchOverlapArtifact::new(
        record.id(),
        values.footprints,
        values.overlap,
        context.verified_footprints,
        context.graph,
    )
}

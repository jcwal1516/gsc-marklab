use std::io::{Cursor, Read, Seek};

use arrow::array::{Array, Int64Array, StringArray};
use arrow_ipc::reader::FileReaderBuilder;
use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};

use crate::{
    ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph,
    VerifiedDirectPatchEmbeddingArtifactGraph, VerifiedPatchFootprintArtifact,
    VerifiedPatchOverlapArtifact,
};

use super::{
    preflight::{
        preflight_patch_footprint_set_arrow_bytes, preflight_patch_overlap_graph_arrow_bytes,
        preflight_reader, PatchFootprintArrowPreflight, PatchOverlapArrowPreflight,
    },
    profile::{arrow_failure, expected_schema, SpatialArrowProfile},
};
use crate::{
    columnar::{
        multiscale::{enforce_file_budget, enforce_retained_budget},
        EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialArrowFailure,
    },
    multiscale::physical::{record_matches_encoding, SpatialArtifactRole, SpatialPhysicalEncoding},
};

/// Fully decode and validate borrowed canonical footprint Arrow bytes.
pub fn validate_patch_footprint_set_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchFootprintArrowPreflight, MultiscaleColumnarError> {
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(
        record,
        encoded_len,
        SpatialArtifactRole::Footprint,
        footprints.row_count(),
        footprint_dependencies(footprints),
    )?;
    let preflight =
        preflight_patch_footprint_set_arrow_bytes(bytes, expected, context, footprints, budgets)?;
    validate_identity(
        preflight.content_digest(),
        preflight.encoded_byte_len(),
        record,
    )?;
    decode_arrow(
        Cursor::new(bytes),
        SpatialArrowProfile::Footprint,
        expected,
        context,
        footprints,
        None,
        preflight.retained_preflight_bytes,
        preflight.maximum_batch_decoded_bytes,
        budgets,
    )?;
    Ok(preflight)
}

/// Fully decode and validate borrowed canonical overlap Arrow bytes.
pub fn validate_patch_overlap_graph_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchOverlapArrowPreflight, MultiscaleColumnarError> {
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(
        record,
        encoded_len,
        SpatialArtifactRole::Overlap,
        overlap.edge_count(),
        overlap_dependencies(footprints, overlap),
    )?;
    let preflight = preflight_patch_overlap_graph_arrow_bytes(
        bytes, expected, context, footprints, overlap, budgets,
    )?;
    validate_identity(
        preflight.content_digest(),
        preflight.encoded_byte_len(),
        record,
    )?;
    decode_arrow(
        Cursor::new(bytes),
        SpatialArrowProfile::Overlap,
        expected,
        context,
        footprints,
        Some(overlap),
        preflight.retained_preflight_bytes,
        preflight.maximum_batch_decoded_bytes,
        budgets,
    )?;
    Ok(preflight)
}

/// Fully decode a managed footprint Arrow artifact inside one integrity envelope.
pub fn validate_patch_footprint_set_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchFootprintArrowPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        let encoded_len = reader
            .seek(std::io::SeekFrom::End(0))
            .map_err(|_| arrow_failure(SpatialArrowFailure::ArtifactRead))?;
        enforce_file_budget(encoded_len, budgets)?;
        validate_record(
            record,
            encoded_len,
            SpatialArtifactRole::Footprint,
            footprints.row_count(),
            footprint_dependencies(footprints),
        )?;
        let common = preflight_reader(
            reader,
            record.content().digest(),
            SpatialArrowProfile::Footprint,
            expected,
            context,
            footprints,
            None,
            budgets,
        )?;
        validate_identity(common.content_digest, common.encoded_byte_len, record)?;
        decode_arrow(
            reader,
            SpatialArrowProfile::Footprint,
            expected,
            context,
            footprints,
            None,
            common.retained_preflight_bytes,
            common.maximum_batch_decoded_bytes,
            budgets,
        )?;
        Ok(PatchFootprintArrowPreflight::from_common(common))
    })
}

/// Fully decode a managed overlap Arrow artifact inside one integrity envelope.
pub fn validate_patch_overlap_graph_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchOverlapArrowPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        let encoded_len = reader
            .seek(std::io::SeekFrom::End(0))
            .map_err(|_| arrow_failure(SpatialArrowFailure::ArtifactRead))?;
        enforce_file_budget(encoded_len, budgets)?;
        validate_record(
            record,
            encoded_len,
            SpatialArtifactRole::Overlap,
            overlap.edge_count(),
            overlap_dependencies(footprints, overlap),
        )?;
        let common = preflight_reader(
            reader,
            record.content().digest(),
            SpatialArrowProfile::Overlap,
            expected,
            context,
            footprints,
            Some(overlap),
            budgets,
        )?;
        validate_identity(common.content_digest, common.encoded_byte_len, record)?;
        decode_arrow(
            reader,
            SpatialArrowProfile::Overlap,
            expected,
            context,
            footprints,
            Some(overlap),
            common.retained_preflight_bytes,
            common.maximum_batch_decoded_bytes,
            budgets,
        )?;
        Ok(PatchOverlapArrowPreflight::from_common(common))
    })
}

/// Verify borrowed footprint Arrow bytes and mint a graph-bound runtime receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_footprint_set_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchFootprintArtifact, MultiscaleColumnarError> {
    validate_patch_footprint_set_arrow_bytes(
        bytes, record, expected, context, footprints, budgets,
    )?;
    VerifiedPatchFootprintArtifact::new(record.id(), context, footprints, graph)
}

/// Verify a managed footprint Arrow artifact and mint a graph-bound runtime receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_footprint_set_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchFootprintArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_patch_footprint_set_arrow_from_store(
        store, record, expected, context, footprints, budgets,
    )?;
    VerifiedPatchFootprintArtifact::new(record.id(), context, footprints, graph)
        .map_err(VerifiedReaderError::Callback)
}

/// Verify borrowed overlap Arrow bytes and mint a footprint- and graph-bound receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_overlap_graph_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    verified_footprints: VerifiedPatchFootprintArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchOverlapArtifact, MultiscaleColumnarError> {
    validate_patch_overlap_graph_arrow_bytes(
        bytes, record, expected, context, footprints, overlap, budgets,
    )?;
    VerifiedPatchOverlapArtifact::new(record.id(), footprints, overlap, verified_footprints, graph)
}

/// Verify a managed overlap Arrow artifact and mint a footprint- and graph-bound receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_overlap_graph_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    verified_footprints: VerifiedPatchFootprintArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchOverlapArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_patch_overlap_graph_arrow_from_store(
        store, record, expected, context, footprints, overlap, budgets,
    )?;
    VerifiedPatchOverlapArtifact::new(record.id(), footprints, overlap, verified_footprints, graph)
        .map_err(VerifiedReaderError::Callback)
}

#[allow(clippy::too_many_arguments)]
fn decode_arrow<R: Read + Seek>(
    source: R,
    profile: SpatialArrowProfile,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: Option<&PatchOverlapGraph>,
    retained_preflight_bytes: usize,
    maximum_batch_decoded_bytes: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    let retained = retained_preflight_bytes
        .checked_add(maximum_batch_decoded_bytes)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained, budgets)?;
    let schema = expected_schema(profile, expected, context, footprints, overlap)?;
    let mut reader = FileReaderBuilder::new()
        .with_max_footer_fb_depth(8)
        .with_max_footer_fb_tables(32)
        .build(source)
        .map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
    if reader.schema().as_ref() != &schema || !reader.custom_metadata().is_empty() {
        return Err(arrow_failure(SpatialArrowFailure::StockDecode));
    }
    let mut global_row = 0_usize;
    for decoded in &mut reader {
        let batch = decoded.map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
        if batch.num_columns() != profile.field_count() {
            return Err(arrow_failure(SpatialArrowFailure::StockDecode));
        }
        for column in batch.columns() {
            column
                .to_data()
                .validate_full()
                .map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
        }
        match profile {
            SpatialArrowProfile::Footprint => {
                let ids = string_column(&batch, 0)?;
                let x = i64_column(&batch, 1)?;
                let y = i64_column(&batch, 2)?;
                if ids.null_count() != 0 || x.null_count() != 0 || y.null_count() != 0 {
                    return Err(arrow_failure(SpatialArrowFailure::StockDecode));
                }
                for local in 0..batch.num_rows() {
                    let row = footprints
                        .footprints()
                        .get(global_row)
                        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidRowCount))?;
                    if ids.value(local) != row.patch_id().as_str()
                        || x.value(local) != row.origin_px()[0]
                        || y.value(local) != row.origin_px()[1]
                    {
                        return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
                    }
                    global_row = global_row
                        .checked_add(1)
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
                }
            }
            SpatialArrowProfile::Overlap => {
                let left = string_column(&batch, 0)?;
                let right = string_column(&batch, 1)?;
                if left.null_count() != 0 || right.null_count() != 0 {
                    return Err(arrow_failure(SpatialArrowFailure::StockDecode));
                }
                let overlap = overlap.ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?;
                for local in 0..batch.num_rows() {
                    let edge = overlap
                        .edges()
                        .get(global_row)
                        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidRowCount))?;
                    if left.value(local) != edge.left_patch_id().as_str()
                        || right.value(local) != edge.right_patch_id().as_str()
                    {
                        return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
                    }
                    global_row = global_row
                        .checked_add(1)
                        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
                }
            }
        }
    }
    let expected_rows = match profile {
        SpatialArrowProfile::Footprint => footprints.row_count(),
        SpatialArrowProfile::Overlap => overlap
            .ok_or(MultiscaleColumnarError::ArtifactBindingMismatch)?
            .edge_count(),
    };
    if global_row != expected_rows {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
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
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::StockDecode))
}

fn i64_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&Int64Array, MultiscaleColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::StockDecode))
}

fn validate_record(
    record: &ArtifactRecord,
    encoded_len: u64,
    role: SpatialArtifactRole,
    row_count: usize,
    dependencies: Vec<marklab_project::ArtifactId>,
) -> Result<(), MultiscaleColumnarError> {
    let row_count = u64::try_from(row_count).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    if record.content().byte_len() != encoded_len
        || record.dependencies() != dependencies
        || !record_matches_encoding(record, role, SpatialPhysicalEncoding::Arrow, row_count)
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    digest: marklab_project::ContentDigest,
    encoded_len: u64,
    record: &ArtifactRecord,
) -> Result<(), MultiscaleColumnarError> {
    if digest != record.content().digest() || encoded_len != record.content().byte_len() {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn footprint_dependencies(footprints: &PatchFootprintSet) -> Vec<marklab_project::ArtifactId> {
    let mut dependencies = vec![
        footprints.expected_patches_artifact_id(),
        footprints.patch_context_artifact_id(),
    ];
    dependencies.sort_unstable();
    dependencies
}

fn overlap_dependencies(
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

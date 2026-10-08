use std::io::{Cursor, Read, Seek};

use arrow::array::{Array, StringArray, UInt64Array};
use arrow_ipc::reader::FileReaderBuilder;
use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};

use crate::{
    columnar::{
        multiscale::{enforce_file_budget, enforce_retained_budget},
        EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialArrowFailure,
    },
    multiscale::physical::{record_matches_encoding, SpatialPhysicalEncoding},
    PatchRegionLink, VerifiedPatchRegionInputArtifactGraph, VerifiedPatchRegionLinkArtifact,
};

use super::{
    preflight::{
        preflight_patch_region_link_arrow_bytes, preflight_reader, PatchRegionArrowPreflight,
    },
    profile::{arrow_failure, dependencies, role, schema, FIELD_COUNT},
};

/// Fully decode and validate borrowed canonical patch-region Arrow bytes.
pub fn validate_patch_region_link_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchRegionArrowPreflight, MultiscaleColumnarError> {
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, link)?;
    let preflight = preflight_patch_region_link_arrow_bytes(bytes, link, budgets)?;
    validate_identity(&preflight, record)?;
    decode(
        Cursor::new(bytes),
        link,
        preflight.retained_preflight_bytes,
        preflight.maximum_batch_decoded_bytes,
        budgets,
    )?;
    Ok(preflight)
}

/// Fully decode one managed patch-region Arrow artifact inside one integrity envelope.
pub fn validate_patch_region_link_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchRegionArrowPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        let encoded_len = reader
            .seek(std::io::SeekFrom::End(0))
            .map_err(|_| arrow_failure(SpatialArrowFailure::ArtifactRead))?;
        enforce_file_budget(encoded_len, budgets)?;
        validate_record(record, encoded_len, link)?;
        let preflight = preflight_reader(reader, record.content().digest(), link, budgets)?;
        validate_identity(&preflight, record)?;
        decode(
            reader,
            link,
            preflight.retained_preflight_bytes,
            preflight.maximum_batch_decoded_bytes,
            budgets,
        )?;
        Ok(preflight)
    })
}

/// Verify borrowed patch-region Arrow bytes and mint a graph-bound runtime receipt.
pub fn verify_patch_region_link_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    link: &PatchRegionLink,
    graph: VerifiedPatchRegionInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchRegionLinkArtifact, MultiscaleColumnarError> {
    validate_patch_region_link_arrow_bytes(bytes, record, link, budgets)?;
    VerifiedPatchRegionLinkArtifact::new(record.id(), link, graph)
}

/// Verify a managed patch-region Arrow artifact and mint a graph-bound runtime receipt.
pub fn verify_patch_region_link_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    link: &PatchRegionLink,
    graph: VerifiedPatchRegionInputArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchRegionLinkArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_patch_region_link_arrow_from_store(store, record, link, budgets)?;
    VerifiedPatchRegionLinkArtifact::new(record.id(), link, graph)
        .map_err(VerifiedReaderError::Callback)
}

fn decode<R: Read + Seek>(
    source: R,
    link: &PatchRegionLink,
    retained_preflight: usize,
    maximum_batch_decoded: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    let retained = retained_preflight
        .checked_add(maximum_batch_decoded)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained, budgets)?;
    let expected_schema = schema(link)?;
    let mut reader = FileReaderBuilder::new()
        .with_max_footer_fb_depth(8)
        .with_max_footer_fb_tables(32)
        .build(source)
        .map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
    if reader.schema().as_ref() != &expected_schema || !reader.custom_metadata().is_empty() {
        return Err(arrow_failure(SpatialArrowFailure::StockDecode));
    }
    let mut global_row = 0_usize;
    for decoded in &mut reader {
        let batch = decoded.map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
        if batch.num_columns() != FIELD_COUNT {
            return Err(arrow_failure(SpatialArrowFailure::StockDecode));
        }
        for column in batch.columns() {
            column
                .to_data()
                .validate_full()
                .map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
        }
        let patches = string_column(&batch, 0)?;
        let regions = string_column(&batch, 1)?;
        let relations = string_column(&batch, 2)?;
        let numerators = u64_column(&batch, 3)?;
        let denominators = u64_column(&batch, 4)?;
        if batch
            .columns()
            .iter()
            .any(|column| column.null_count() != 0)
        {
            return Err(arrow_failure(SpatialArrowFailure::StockDecode));
        }
        for local in 0..batch.num_rows() {
            let row = link
                .nonzero_relations()
                .get(global_row)
                .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidRowCount))?;
            if patches.value(local) != row.patch_id().as_str()
                || regions.value(local) != row.region_id().as_str()
                || relations.value(local) != row.relation().wire_name()
                || numerators.value(local) != row.numerator()
                || denominators.value(local) != row.denominator()
            {
                return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
            }
            global_row = global_row
                .checked_add(1)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        }
    }
    if global_row != link.nonzero_relation_count() {
        return Err(arrow_failure(SpatialArrowFailure::InvalidRowCount));
    }
    Ok(())
}

fn validate_record(
    record: &ArtifactRecord,
    encoded_len: u64,
    link: &PatchRegionLink,
) -> Result<(), MultiscaleColumnarError> {
    let rows = u64::try_from(link.nonzero_relation_count())
        .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    if !record_matches_encoding(record, role(), SpatialPhysicalEncoding::Arrow, rows)
        || record.dependencies() != dependencies(link)
        || record.content().byte_len() != encoded_len
    {
        return Err(MultiscaleColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    preflight: &PatchRegionArrowPreflight,
    record: &ArtifactRecord,
) -> Result<(), MultiscaleColumnarError> {
    if preflight.content_digest() != record.content().digest()
        || preflight.encoded_byte_len() != record.content().byte_len()
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
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::StockDecode))
}

fn u64_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&UInt64Array, MultiscaleColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::StockDecode))
}

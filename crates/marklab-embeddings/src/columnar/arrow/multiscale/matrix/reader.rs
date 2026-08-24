use std::io::{Cursor, Read, Seek, SeekFrom};

use arrow::array::{Array, FixedSizeListArray, Float32Array, StringArray};
use arrow_ipc::reader::FileReaderBuilder;
use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};

use crate::{
    PatchEmbeddingSourceRowLink, PatchEmbeddingTable, RegionEmbeddingTable, SlideEmbeddingTable,
    VerifiedDirectPatchEmbeddingArtifactGraph, VerifiedPatchEmbeddingSupportArtifact,
    VerifiedPatchEmbeddingTableArtifact,
};

use super::{
    preflight::{preflight_reader, MultiscaleMatrixArrowPreflight},
    profile::{arrow_failure, schema, COLUMN_COUNT},
};
use crate::columnar::{
    multiscale::{
        enforce_file_budget, enforce_retained_budget, matrix_dependencies,
        MatrixPhysicalAccumulator, MultiscaleMatrixTable,
    },
    EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialArrowFailure,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

macro_rules! typed_reader {
    ($bytes:ident, $managed:ident, $table:ty) => {
        #[doc = "Fully decode and validate borrowed canonical C-05 Arrow matrix bytes."]
        pub fn $bytes(
            bytes: &[u8],
            record: &ArtifactRecord,
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<MultiscaleMatrixArrowPreflight, MultiscaleColumnarError> {
            validate_bytes(bytes, record, table, budgets)
        }

        #[doc = "Fully decode one managed C-05 Arrow matrix inside one integrity envelope."]
        pub fn $managed(
            store: &LocalArtifactStore,
            record: &ArtifactRecord,
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<MultiscaleMatrixArrowPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
            validate_from_store(store, record, table, budgets)
        }
    };
}

typed_reader!(
    validate_patch_embedding_table_arrow_bytes,
    validate_patch_embedding_table_arrow_from_store,
    PatchEmbeddingTable
);

/// Fully decode borrowed patch-matrix Arrow bytes and mint an exact direct-patch receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_embedding_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    table: &PatchEmbeddingTable,
    source_row_link: &PatchEmbeddingSourceRowLink,
    support: VerifiedPatchEmbeddingSupportArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchEmbeddingTableArtifact, MultiscaleColumnarError> {
    validate_patch_embedding_table_arrow_bytes(bytes, record, table, budgets)?;
    VerifiedPatchEmbeddingTableArtifact::new(record.id(), table, source_row_link, support, graph)
}

/// Fully decode a managed patch-matrix Arrow artifact and mint an exact direct-patch receipt.
#[allow(clippy::too_many_arguments)]
pub fn verify_patch_embedding_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    table: &PatchEmbeddingTable,
    source_row_link: &PatchEmbeddingSourceRowLink,
    support: VerifiedPatchEmbeddingSupportArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchEmbeddingTableArtifact, VerifiedReaderError<MultiscaleColumnarError>> {
    validate_patch_embedding_table_arrow_from_store(store, record, table, budgets)?;
    VerifiedPatchEmbeddingTableArtifact::new(record.id(), table, source_row_link, support, graph)
        .map_err(VerifiedReaderError::Callback)
}
typed_reader!(
    validate_region_embedding_table_arrow_bytes,
    validate_region_embedding_table_arrow_from_store,
    RegionEmbeddingTable
);
typed_reader!(
    validate_slide_embedding_table_arrow_bytes,
    validate_slide_embedding_table_arrow_from_store,
    SlideEmbeddingTable
);

fn validate_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    table: &dyn MultiscaleMatrixTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<MultiscaleMatrixArrowPreflight, MultiscaleColumnarError> {
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, table)?;
    let mut source = Cursor::new(bytes);
    let preflight = preflight_reader(
        &mut source,
        marklab_project::ContentDigest::from_bytes(bytes),
        table,
        budgets,
    )?;
    validate_identity(preflight, record)?;
    decode_arrow(source, table, preflight, budgets)?;
    Ok(preflight)
}

fn validate_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    table: &dyn MultiscaleMatrixTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<MultiscaleMatrixArrowPreflight, VerifiedReaderError<MultiscaleColumnarError>> {
    store.with_verified_reader(record, |reader| {
        let encoded_len = reader
            .seek(SeekFrom::End(0))
            .map_err(|_| arrow_failure(SpatialArrowFailure::ArtifactRead))?;
        enforce_file_budget(encoded_len, budgets)?;
        validate_record(record, encoded_len, table)?;
        let preflight = preflight_reader(reader, record.content().digest(), table, budgets)?;
        validate_identity(preflight, record)?;
        decode_arrow(reader, table, preflight, budgets)?;
        Ok(preflight)
    })
}

fn decode_arrow<R: Read + Seek>(
    source: R,
    table: &dyn MultiscaleMatrixTable,
    preflight: MultiscaleMatrixArrowPreflight,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), MultiscaleColumnarError> {
    let retained = preflight
        .retained_preflight_bytes
        .checked_add(preflight.maximum_batch_decoded_bytes)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(MultiscaleColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained, budgets)?;
    let expected_schema = schema(table)?;
    let mut reader = FileReaderBuilder::new()
        .with_max_footer_fb_depth(8)
        .with_max_footer_fb_tables(32)
        .build(source)
        .map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
    if reader.schema().as_ref() != &expected_schema || !reader.custom_metadata().is_empty() {
        return Err(arrow_failure(SpatialArrowFailure::StockDecode));
    }
    let mut accumulator = MatrixPhysicalAccumulator::new(table)?;
    let mut global_row = 0_usize;
    for decoded in &mut reader {
        let batch = decoded.map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
        if batch.num_columns() != COLUMN_COUNT {
            return Err(arrow_failure(SpatialArrowFailure::StockDecode));
        }
        for column in batch.columns() {
            column
                .to_data()
                .validate_full()
                .map_err(|_| arrow_failure(SpatialArrowFailure::StockDecode))?;
        }
        let ids = string_column(&batch, 0)?;
        let embeddings = batch
            .column(1)
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::StockDecode))?;
        let values = embeddings
            .values()
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::StockDecode))?;
        let statuses = string_column(&batch, 2)?;
        let expected_dimension =
            i32::try_from(table.dimension()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
        if ids.null_count() != 0
            || embeddings.null_count() != 0
            || values.null_count() != 0
            || statuses.null_count() != 0
            || embeddings.value_length() != expected_dimension
        {
            return Err(arrow_failure(SpatialArrowFailure::StockDecode));
        }
        let dimension = usize::try_from(table.dimension())
            .map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
        for local in 0..batch.num_rows() {
            let expected = table
                .row(global_row)
                .map_err(|_| arrow_failure(SpatialArrowFailure::InvalidRowCount))?;
            let value_start = local
                .checked_mul(dimension)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            let value_end = value_start
                .checked_add(dimension)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
            let vector = values
                .values()
                .get(value_start..value_end)
                .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidCanonicalRows))?;
            if ids.value(local) != expected.id()
                || statuses.value(local) != expected.status().wire_name()
                || vector.iter().copied().enumerate().any(|(column, value)| {
                    value.to_bits()
                        != expected
                            .vector()
                            .map_or(0.0, |expected_vector| expected_vector[column])
                            .to_bits()
                })
            {
                return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
            }
            let observed_vector = expected.vector().map(|_| vector);
            accumulator.push(expected.id(), expected.status(), observed_vector)?;
            global_row = global_row
                .checked_add(1)
                .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        }
    }
    if global_row != table.row_count()
        || accumulator.finish()? != preflight.qc_summary()
        || preflight.qc_summary() != table.qc_summary()
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidCanonicalRows));
    }
    Ok(())
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
        SpatialPhysicalEncoding::Arrow,
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
    preflight: MultiscaleMatrixArrowPreflight,
    record: &ArtifactRecord,
) -> Result<(), MultiscaleColumnarError> {
    if preflight.encoded_byte_len() != record.content().byte_len()
        || preflight.content_digest() != record.content().digest()
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

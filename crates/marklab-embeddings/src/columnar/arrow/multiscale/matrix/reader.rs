use std::io::{Cursor, Read, Seek, SeekFrom};

use arrow::array::{Array, FixedSizeListArray, Float32Array, StringArray};
use arrow_ipc::reader::FileReaderBuilder;
use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};

use crate::{
    DerivedRegionEmbeddingTableCandidate, DerivedSlideEmbeddingTableCandidate, ExpectedPatchSet,
    MultiscaleEmbeddingTableReadBindings, PatchEmbeddingSourceRowLink, PatchEmbeddingTable,
    RegionEmbeddingTable, SlideEmbeddingTable, VerifiedDirectPatchEmbeddingArtifactGraph,
    VerifiedPatchEmbeddingSupportArtifact, VerifiedPatchEmbeddingTableArtifact,
    VerifiedRegionEmbeddingTableArtifact, VerifiedSlideEmbeddingTableArtifact,
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
    ArrowIpcFailure, ColumnarError, EmbeddingColumnarBudgets,
};
use crate::multiscale::physical::SpatialPhysicalEncoding;

macro_rules! typed_reader {
    ($bytes:ident, $managed:ident, $table:ty) => {
        #[doc = "Fully decode and validate borrowed canonical multiscale Arrow matrix bytes."]
        pub fn $bytes(
            bytes: &[u8],
            record: &ArtifactRecord,
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<MultiscaleMatrixArrowPreflight, ColumnarError> {
            validate_bytes(bytes, record, table, budgets)
        }

        #[doc = "Fully decode one managed multiscale Arrow matrix inside one integrity envelope."]
        pub fn $managed(
            store: &LocalArtifactStore,
            record: &ArtifactRecord,
            table: &$table,
            budgets: EmbeddingColumnarBudgets,
        ) -> Result<MultiscaleMatrixArrowPreflight, VerifiedReaderError<ColumnarError>> {
            validate_from_store(store, record, table, budgets)
        }
    };
}

typed_reader!(
    validate_patch_embedding_table_arrow_bytes,
    validate_patch_embedding_table_arrow_from_store,
    PatchEmbeddingTable
);

/// Materialize fully verified canonical patch-matrix Arrow bytes.
pub fn read_patch_embedding_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    bindings: MultiscaleEmbeddingTableReadBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchEmbeddingTable, ColumnarError> {
    crate::columnar::read::<crate::columnar::ArrowIpc, crate::columnar::PatchEmbeddingTableEntity>(
        Cursor::new(bytes),
        record,
        &crate::columnar::PatchEmbeddingTableReadContext { expected, bindings },
        budgets,
    )
}

/// Materialize a managed canonical patch-matrix Arrow artifact through one integrity envelope.
pub fn read_patch_embedding_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    bindings: MultiscaleEmbeddingTableReadBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<PatchEmbeddingTable, VerifiedReaderError<ColumnarError>> {
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.read::<crate::columnar::ArrowIpc, crate::columnar::PatchEmbeddingTableEntity>(
            &crate::columnar::PatchEmbeddingTableReadContext { expected, bindings },
        )
    })
}

/// Fully decode borrowed patch-matrix Arrow bytes and mint an exact direct-patch receipt.
pub fn verify_patch_embedding_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    table: &PatchEmbeddingTable,
    source_row_link: &PatchEmbeddingSourceRowLink,
    support: VerifiedPatchEmbeddingSupportArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchEmbeddingTableArtifact, ColumnarError> {
    crate::columnar::verify::<crate::columnar::ArrowIpc, crate::columnar::PatchEmbeddingTableEntity>(
        Cursor::new(bytes),
        record,
        &crate::columnar::PatchEmbeddingTableVerifyContext {
            table,
            source_row_link,
            support,
            graph,
        },
        budgets,
    )
}

/// Fully decode a managed patch-matrix Arrow artifact and mint an exact direct-patch receipt.
pub fn verify_patch_embedding_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    table: &PatchEmbeddingTable,
    source_row_link: &PatchEmbeddingSourceRowLink,
    support: VerifiedPatchEmbeddingSupportArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedPatchEmbeddingTableArtifact, VerifiedReaderError<ColumnarError>> {
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.verify::<crate::columnar::ArrowIpc, crate::columnar::PatchEmbeddingTableEntity>(
            &crate::columnar::PatchEmbeddingTableVerifyContext {
                table,
                source_row_link,
                support,
                graph,
            },
        )
    })
}
typed_reader!(
    validate_region_embedding_table_arrow_bytes,
    validate_region_embedding_table_arrow_from_store,
    RegionEmbeddingTable
);

/// Fully decode borrowed derived-region Arrow bytes and mint an exact finalization receipt.
pub fn verify_region_embedding_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    candidate: &DerivedRegionEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedRegionEmbeddingTableArtifact, ColumnarError> {
    crate::columnar::verify::<crate::columnar::ArrowIpc, crate::columnar::RegionEmbeddingTableEntity>(
        Cursor::new(bytes),
        record,
        &candidate,
        budgets,
    )
}

/// Fully decode a managed derived-region Arrow artifact and mint an exact finalization receipt.
pub fn verify_region_embedding_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    candidate: &DerivedRegionEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedRegionEmbeddingTableArtifact, VerifiedReaderError<ColumnarError>> {
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.verify::<crate::columnar::ArrowIpc, crate::columnar::RegionEmbeddingTableEntity>(
            &candidate,
        )
    })
}
typed_reader!(
    validate_slide_embedding_table_arrow_bytes,
    validate_slide_embedding_table_arrow_from_store,
    SlideEmbeddingTable
);

/// Fully decode borrowed derived-slide Arrow bytes and mint an exact finalization receipt.
pub fn verify_slide_embedding_table_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    candidate: &DerivedSlideEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedSlideEmbeddingTableArtifact, ColumnarError> {
    crate::columnar::verify::<crate::columnar::ArrowIpc, crate::columnar::SlideEmbeddingTableEntity>(
        Cursor::new(bytes),
        record,
        &candidate,
        budgets,
    )
}

/// Fully decode a managed derived-slide Arrow artifact and mint an exact finalization receipt.
pub fn verify_slide_embedding_table_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    candidate: &DerivedSlideEmbeddingTableCandidate,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedSlideEmbeddingTableArtifact, VerifiedReaderError<ColumnarError>> {
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.verify::<crate::columnar::ArrowIpc, crate::columnar::SlideEmbeddingTableEntity>(
            &candidate,
        )
    })
}

fn validate_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    table: &dyn MultiscaleMatrixTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<MultiscaleMatrixArrowPreflight, ColumnarError> {
    let encoded_len = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
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
) -> Result<MultiscaleMatrixArrowPreflight, VerifiedReaderError<ColumnarError>> {
    store.with_verified_reader(record, |reader| {
        validate_reader(reader, record, table, budgets)
    })
}

pub(crate) fn validate_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    table: &dyn MultiscaleMatrixTable,
    budgets: EmbeddingColumnarBudgets,
) -> Result<MultiscaleMatrixArrowPreflight, ColumnarError> {
    let encoded_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))?;
    enforce_file_budget(encoded_len, budgets)?;
    validate_record(record, encoded_len, table)?;
    let preflight = preflight_reader(reader, record.content().digest(), table, budgets)?;
    validate_identity(preflight, record)?;
    decode_arrow(reader, table, preflight, budgets)?;
    Ok(preflight)
}

fn decode_arrow<R: Read + Seek>(
    source: R,
    table: &dyn MultiscaleMatrixTable,
    preflight: MultiscaleMatrixArrowPreflight,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), ColumnarError> {
    let retained = preflight
        .retained_preflight_bytes
        .checked_add(preflight.maximum_batch_decoded_bytes)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(ColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained, budgets)?;
    let expected_schema = schema(table)?;
    let mut reader = FileReaderBuilder::new()
        .with_max_footer_fb_depth(8)
        .with_max_footer_fb_tables(32)
        .build(source)
        .map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
    if reader.schema().as_ref() != &expected_schema || !reader.custom_metadata().is_empty() {
        return Err(arrow_failure(ArrowIpcFailure::StockDecode));
    }
    let mut accumulator = MatrixPhysicalAccumulator::new(table)?;
    let mut global_row = 0_usize;
    for decoded in &mut reader {
        let batch = decoded.map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
        if batch.num_columns() != COLUMN_COUNT {
            return Err(arrow_failure(ArrowIpcFailure::StockDecode));
        }
        for column in batch.columns() {
            column
                .to_data()
                .validate_full()
                .map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
        }
        let ids = string_column(&batch, 0)?;
        let embeddings = batch
            .column(1)
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
        let values = embeddings
            .values()
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
        let statuses = string_column(&batch, 2)?;
        let expected_dimension =
            i32::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)?;
        if ids.null_count() != 0
            || embeddings.null_count() != 0
            || values.null_count() != 0
            || statuses.null_count() != 0
            || embeddings.value_length() != expected_dimension
        {
            return Err(arrow_failure(ArrowIpcFailure::StockDecode));
        }
        let dimension =
            usize::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)?;
        for local in 0..batch.num_rows() {
            let expected = table
                .row(global_row)
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidRowCount))?;
            let value_start = local
                .checked_mul(dimension)
                .ok_or(ColumnarError::SizeOverflow)?;
            let value_end = value_start
                .checked_add(dimension)
                .ok_or(ColumnarError::SizeOverflow)?;
            let vector = values
                .values()
                .get(value_start..value_end)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidCanonicalRows))?;
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
                return Err(arrow_failure(ArrowIpcFailure::InvalidCanonicalRows));
            }
            let observed_vector = expected.vector().map(|_| vector);
            accumulator.push(expected.id(), expected.status(), observed_vector)?;
            global_row = global_row
                .checked_add(1)
                .ok_or(ColumnarError::SizeOverflow)?;
        }
    }
    if global_row != table.row_count()
        || accumulator.finish()? != preflight.qc_summary()
        || preflight.qc_summary() != table.qc_summary()
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidCanonicalRows));
    }
    Ok(())
}

fn validate_record(
    record: &ArtifactRecord,
    encoded_len: u64,
    table: &dyn MultiscaleMatrixTable,
) -> Result<(), ColumnarError> {
    let profile = table.profile();
    let row_count = u64::try_from(table.row_count()).map_err(|_| ColumnarError::SizeOverflow)?;
    if !profile.record_matches_encoding(
        record,
        SpatialPhysicalEncoding::Arrow,
        row_count,
        table.dimension(),
    ) || record.content().byte_len() != encoded_len
        || record.dependencies() != matrix_dependencies(table)?
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn validate_identity(
    preflight: MultiscaleMatrixArrowPreflight,
    record: &ArtifactRecord,
) -> Result<(), ColumnarError> {
    if preflight.encoded_byte_len() != record.content().byte_len()
        || preflight.content_digest() != record.content().digest()
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn string_column(
    batch: &arrow::record_batch::RecordBatch,
    index: usize,
) -> Result<&StringArray, ColumnarError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))
}

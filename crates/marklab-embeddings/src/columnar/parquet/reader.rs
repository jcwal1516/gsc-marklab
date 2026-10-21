use std::io::{Cursor, Read, Seek};

use marklab_project::{ArtifactRecord, LocalArtifactStore, VerifiedReaderError};

use crate::{
    CellEmbeddingRowLink, CellEmbeddingTable, EmbeddingQcSummary, ExpectedCellSet,
    VerifiedCellEmbeddingArtifactGraph, VerifiedCellEmbeddingTableArtifact,
};

use self::{
    decode::{materialize_cell_embedding_table_parquet, scan_cell_embedding_table_parquet},
    record::validate_embedding_record,
};
use super::{
    super::{CellEmbeddingTablePhysicalBindings, ColumnarError, EmbeddingColumnarBudgets},
    preflight::{
        declared_table_logical_digest_parquet_reader, prepare_cell_embedding_table_parquet_reader,
        PreparedCellEmbeddingParquet,
    },
};

mod decode;
mod record;
mod resources;
mod window;

pub(super) use window::RowGroupWindow;

/// Materialize fully preflighted borrowed Parquet bytes after provenance-graph validation.
pub fn read_cell_embedding_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingTable, ColumnarError> {
    let context = crate::CellEmbeddingTableReadContext {
        expected,
        row_link,
        graph,
    };
    crate::columnar::read::<crate::Parquet, crate::CellEmbeddingTableEntity>(
        Cursor::new(bytes),
        record,
        &context,
        budgets,
    )
}

/// Materialize a managed Parquet table through one pre/post-verified store descriptor.
pub fn read_cell_embedding_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingTable, VerifiedReaderError<ColumnarError>> {
    let context = crate::CellEmbeddingTableReadContext {
        expected,
        row_link,
        graph,
    };
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.read::<crate::Parquet, crate::CellEmbeddingTableEntity>(&context)
    })
}

/// Stream-scan fully preflighted borrowed Parquet bytes without retaining the table.
pub fn scan_cell_embedding_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<EmbeddingQcSummary, ColumnarError> {
    let encoded_byte_len = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(ColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    let dimension = validate_embedding_record(record, encoded_byte_len, expected, row_link, graph)?;
    let mut source = Cursor::new(bytes);
    let declared_logical_digest = declared_table_logical_digest_parquet_reader(
        &mut source,
        encoded_byte_len,
        expected,
        budgets,
    )?;
    let bindings = physical_bindings(graph, declared_logical_digest)?;
    let prepared = prepare_cell_embedding_table_parquet_reader(
        &mut source,
        encoded_byte_len,
        marklab_project::ContentDigest::from_bytes(bytes),
        expected,
        dimension,
        bindings,
        budgets,
    )?;
    validate_preflight_identity(&prepared, record, dimension)?;
    scan_cell_embedding_table_parquet(
        &mut source,
        expected,
        row_link,
        graph,
        dimension,
        declared_logical_digest,
        prepared,
        budgets,
    )
}

/// Stream-scan a managed Parquet table through one pre/post-verified descriptor.
pub fn scan_cell_embedding_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<EmbeddingQcSummary, VerifiedReaderError<ColumnarError>> {
    let encoded_byte_len = record.content().byte_len();
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(VerifiedReaderError::Callback(
            ColumnarError::FileByteBudgetExceeded {
                observed: encoded_byte_len,
                maximum: budgets.maximum_file_bytes(),
            },
        ));
    }
    let dimension = validate_embedding_record(record, encoded_byte_len, expected, row_link, graph)
        .map_err(VerifiedReaderError::Callback)?;
    store.with_verified_reader(record, |reader| {
        let declared_logical_digest = declared_table_logical_digest_parquet_reader(
            reader,
            encoded_byte_len,
            expected,
            budgets,
        )?;
        let bindings = physical_bindings(graph, declared_logical_digest)?;
        let prepared = prepare_cell_embedding_table_parquet_reader(
            reader,
            encoded_byte_len,
            record.content().digest(),
            expected,
            dimension,
            bindings,
            budgets,
        )?;
        validate_preflight_identity(&prepared, record, dimension)?;
        scan_cell_embedding_table_parquet(
            reader,
            expected,
            row_link,
            graph,
            dimension,
            declared_logical_digest,
            prepared,
            budgets,
        )
    })
}

/// Fully verify borrowed Parquet bytes and return a receipt bound to the exact artifact record.
pub fn verify_cell_embedding_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellEmbeddingTableArtifact, ColumnarError> {
    let context = crate::CellEmbeddingTableReadContext {
        expected,
        row_link,
        graph,
    };
    crate::columnar::verify::<crate::Parquet, crate::CellEmbeddingTableEntity>(
        Cursor::new(bytes),
        record,
        &context,
        budgets,
    )
}

/// Fully verify a managed Parquet table and return an exact artifact-bound receipt.
pub fn verify_cell_embedding_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellEmbeddingTableArtifact, VerifiedReaderError<ColumnarError>> {
    let context = crate::CellEmbeddingTableReadContext {
        expected,
        row_link,
        graph,
    };
    crate::columnar::with_verified_columnar_reader(store, record, budgets, |reader| {
        reader.verify::<crate::Parquet, crate::CellEmbeddingTableEntity>(&context)
    })
}

fn physical_bindings(
    graph: VerifiedCellEmbeddingArtifactGraph,
    declared_logical_digest: marklab_project::ContentDigest,
) -> Result<CellEmbeddingTablePhysicalBindings, ColumnarError> {
    CellEmbeddingTablePhysicalBindings::new(
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_artifact_id,
        graph.row_link_logical_digest,
        declared_logical_digest,
    )
}

fn validate_preflight_identity(
    prepared: &PreparedCellEmbeddingParquet,
    record: &ArtifactRecord,
    dimension: u32,
) -> Result<(), ColumnarError> {
    if prepared.summary.dimension() != dimension
        || prepared.summary.content_digest() != record.content().digest()
        || prepared.summary.encoded_byte_len() != record.content().byte_len()
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn prepare_trusted_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &crate::CellEmbeddingTableReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<
    (
        u32,
        marklab_project::ContentDigest,
        PreparedCellEmbeddingParquet,
    ),
    ColumnarError,
> {
    let encoded = record.content().byte_len();
    super::super::enforce_file_budget(encoded, budgets)?;
    let dimension = validate_embedding_record(
        record,
        encoded,
        context.expected,
        context.row_link,
        context.graph,
    )?;
    let digest =
        declared_table_logical_digest_parquet_reader(reader, encoded, context.expected, budgets)?;
    let prepared = prepare_cell_embedding_table_parquet_reader(
        reader,
        encoded,
        record.content().digest(),
        context.expected,
        dimension,
        physical_bindings(context.graph, digest)?,
        budgets,
    )?;
    validate_preflight_identity(&prepared, record, dimension)?;
    Ok((dimension, digest, prepared))
}

pub(crate) fn read_cell_embedding_table_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &crate::CellEmbeddingTableReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingTable, ColumnarError> {
    let (dimension, digest, prepared) = prepare_trusted_reader(reader, record, context, budgets)?;
    materialize_cell_embedding_table_parquet(
        reader,
        context.expected,
        context.row_link,
        context.graph,
        dimension,
        digest,
        prepared,
        budgets,
    )
}

pub(crate) fn verify_cell_embedding_table_parquet_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    record: &ArtifactRecord,
    context: &crate::CellEmbeddingTableReadContext<'_>,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellEmbeddingTableArtifact, ColumnarError> {
    let (dimension, digest, prepared) = prepare_trusted_reader(reader, record, context, budgets)?;
    let qc = scan_cell_embedding_table_parquet(
        reader,
        context.expected,
        context.row_link,
        context.graph,
        dimension,
        digest,
        prepared,
        budgets,
    )?;
    Ok(VerifiedCellEmbeddingTableArtifact::new(
        record.id(),
        context.graph,
        qc,
    ))
}

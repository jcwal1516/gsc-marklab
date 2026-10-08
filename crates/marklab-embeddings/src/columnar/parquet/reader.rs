use std::io::Cursor;

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
    super::{CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets, EmbeddingColumnarError},
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
) -> Result<CellEmbeddingTable, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
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
    materialize_cell_embedding_table_parquet(
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

/// Materialize a managed Parquet table through one pre/post-verified store descriptor.
pub fn read_cell_embedding_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingTable, VerifiedReaderError<EmbeddingColumnarError>> {
    let encoded_byte_len = record.content().byte_len();
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::FileByteBudgetExceeded {
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
        materialize_cell_embedding_table_parquet(
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

/// Stream-scan fully preflighted borrowed Parquet bytes without retaining the table.
pub fn scan_cell_embedding_table_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<EmbeddingQcSummary, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
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
) -> Result<EmbeddingQcSummary, VerifiedReaderError<EmbeddingColumnarError>> {
    let encoded_byte_len = record.content().byte_len();
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::FileByteBudgetExceeded {
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
) -> Result<VerifiedCellEmbeddingTableArtifact, EmbeddingColumnarError> {
    let qc_summary =
        scan_cell_embedding_table_parquet_bytes(bytes, record, expected, row_link, graph, budgets)?;
    Ok(VerifiedCellEmbeddingTableArtifact::new(
        record.id(),
        graph,
        qc_summary,
    ))
}

/// Fully verify a managed Parquet table and return an exact artifact-bound receipt.
pub fn verify_cell_embedding_table_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<VerifiedCellEmbeddingTableArtifact, VerifiedReaderError<EmbeddingColumnarError>> {
    let qc_summary = scan_cell_embedding_table_parquet_from_store(
        store, record, expected, row_link, graph, budgets,
    )?;
    Ok(VerifiedCellEmbeddingTableArtifact::new(
        record.id(),
        graph,
        qc_summary,
    ))
}

fn physical_bindings(
    graph: VerifiedCellEmbeddingArtifactGraph,
    declared_logical_digest: marklab_project::ContentDigest,
) -> Result<CellEmbeddingTablePhysicalBindings, EmbeddingColumnarError> {
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
) -> Result<(), EmbeddingColumnarError> {
    if prepared.summary.dimension() != dimension
        || prepared.summary.content_digest() != record.content().digest()
        || prepared.summary.encoded_byte_len() != record.content().byte_len()
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

use std::{collections::BTreeMap, io};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactRef, ArtifactSchema, ArtifactStoreError,
    LocalArtifactStore, TableColumn, TableColumnType, TableFormat, TableManifest, TableScalarType,
};
use thiserror::Error;

use crate::CellEmbeddingTable;

use super::{
    super::super::{
        CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets, EmbeddingColumnarError,
    },
    profile::{ENCODING_VERSION, SCHEMA_ID},
    writer::write_cell_embedding_table_arrow,
};

/// Failure while declaring, encoding, or durably publishing a canonical columnar artifact.
#[derive(Debug, Error)]
pub enum EmbeddingColumnarPublicationError {
    /// Canonical encoding or resource validation failed before publication.
    #[error(transparent)]
    Columnar(#[from] EmbeddingColumnarError),
    /// A fixed canonical artifact or table declaration could not be constructed.
    #[error("canonical embedding artifact declaration failed")]
    Declaration,
    /// Durable store publication, verification, or cleanup failed.
    #[error(transparent)]
    Store(#[from] ArtifactStoreError),
}

/// Count/hash, replay, and durably publish a fresh canonical Arrow embedding table.
pub fn publish_cell_embedding_table_arrow(
    store: &LocalArtifactStore,
    table: &CellEmbeddingTable,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, EmbeddingColumnarPublicationError> {
    let summary = write_cell_embedding_table_arrow(&mut io::sink(), table, bindings, budgets)?;
    let manifest = embedding_table_manifest(summary.row_count(), summary.dimension())?;
    let content = ArtifactRef::new(
        "application/vnd.marklab.cell-embedding-table.v1+arrow",
        summary.content_digest(),
        summary.encoded_byte_len(),
    )
    .map_err(map_declaration_error)?;
    let draft = ArtifactDraft::new(
        content,
        ArtifactSchema::new(SCHEMA_ID, 1).map_err(map_declaration_error)?,
        Some(manifest),
        vec![
            bindings.expected_cells_artifact_id(),
            bindings.provenance_artifact_id(),
            bindings.row_link_artifact_id(),
        ],
        BTreeMap::new(),
    )
    .map_err(map_declaration_error)?;
    store
        .publish_new_send(&draft, |output| {
            write_cell_embedding_table_arrow(output, table, bindings, budgets)
                .map(|_| ())
                .map_err(|_| io::Error::other("canonical Arrow replay failed"))
        })
        .map_err(EmbeddingColumnarPublicationError::Store)
}

fn embedding_table_manifest(
    row_count: u64,
    dimension: u32,
) -> Result<TableManifest, EmbeddingColumnarPublicationError> {
    let columns = vec![
        TableColumn::new(
            "cell_id",
            TableColumnType::Scalar(TableScalarType::Utf8),
            false,
        )
        .map_err(map_declaration_error)?,
        TableColumn::new(
            "embedding",
            TableColumnType::FixedSizeList {
                element: TableScalarType::F32,
                length: dimension,
            },
            false,
        )
        .map_err(map_declaration_error)?,
        TableColumn::new(
            "embedding_status",
            TableColumnType::Scalar(TableScalarType::Utf8),
            false,
        )
        .map_err(map_declaration_error)?,
    ];
    TableManifest::new(
        TableFormat::ArrowIpcFile,
        ENCODING_VERSION,
        row_count,
        columns,
        vec!["cell_id".to_owned()],
    )
    .map_err(map_declaration_error)
}

fn map_declaration_error<T>(_: T) -> EmbeddingColumnarPublicationError {
    EmbeddingColumnarPublicationError::Declaration
}

use std::{collections::BTreeMap, io};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactRef, ArtifactSchema, LocalArtifactStore,
    TableColumn, TableColumnType, TableFormat, TableManifest, TableScalarType,
};

use crate::CellEmbeddingTable;

use super::{
    super::{
        CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets,
        EmbeddingColumnarPublicationError,
    },
    profile::{CONTENT_KIND, ENCODING_VERSION, SCHEMA_ID},
    writer::write_cell_embedding_table_parquet,
};

/// Count/hash, replay, and durably publish a fresh canonical Parquet embedding table.
pub fn publish_cell_embedding_table_parquet(
    store: &LocalArtifactStore,
    table: &CellEmbeddingTable,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, EmbeddingColumnarPublicationError> {
    let summary = write_cell_embedding_table_parquet(&mut io::sink(), table, bindings, budgets)?;
    let content = ArtifactRef::new(
        CONTENT_KIND,
        summary.content_digest(),
        summary.encoded_byte_len(),
    )
    .map_err(map_declaration_error)?;
    let draft = ArtifactDraft::new(
        content,
        ArtifactSchema::new(SCHEMA_ID, 1).map_err(map_declaration_error)?,
        Some(embedding_manifest(
            summary.row_count(),
            summary.dimension(),
        )?),
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
            write_cell_embedding_table_parquet(output, table, bindings, budgets)
                .map(|_| ())
                .map_err(|_| io::Error::other("canonical Parquet replay failed"))
        })
        .map_err(EmbeddingColumnarPublicationError::Store)
}

fn embedding_manifest(
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
        TableFormat::ParquetFile,
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

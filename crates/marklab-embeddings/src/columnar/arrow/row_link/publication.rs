use std::{collections::BTreeMap, io};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactRef, ArtifactSchema, LocalArtifactStore,
    TableColumn, TableColumnType, TableFormat, TableManifest, TableScalarType,
};

use crate::CellEmbeddingRowLink;

use super::{
    super::super::super::{EmbeddingColumnarBudgets, EmbeddingColumnarPublicationError},
    profile::{CONTENT_KIND, ENCODING_VERSION, SCHEMA_ID},
    writer::write_cell_embedding_row_link_arrow,
};

/// Count/hash, replay, and durably publish a fresh canonical Arrow row link.
pub fn publish_cell_embedding_row_link_arrow(
    store: &LocalArtifactStore,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, EmbeddingColumnarPublicationError> {
    let summary = write_cell_embedding_row_link_arrow(&mut io::sink(), row_link, budgets)?;
    let content = ArtifactRef::new(
        CONTENT_KIND,
        summary.content_digest(),
        summary.encoded_byte_len(),
    )
    .map_err(map_declaration_error)?;
    let draft = ArtifactDraft::new(
        content,
        ArtifactSchema::new(SCHEMA_ID, 1).map_err(map_declaration_error)?,
        Some(row_link_manifest(summary.row_count())?),
        row_link.direct_dependencies().to_vec(),
        BTreeMap::new(),
    )
    .map_err(map_declaration_error)?;
    store
        .publish_new_send(&draft, |output| {
            write_cell_embedding_row_link_arrow(output, row_link, budgets)
                .map(|_| ())
                .map_err(|_| io::Error::other("canonical row-link Arrow replay failed"))
        })
        .map_err(EmbeddingColumnarPublicationError::Store)
}

fn row_link_manifest(row_count: u64) -> Result<TableManifest, EmbeddingColumnarPublicationError> {
    let columns = vec![
        TableColumn::new(
            "cell_id",
            TableColumnType::Scalar(TableScalarType::Utf8),
            false,
        )
        .map_err(map_declaration_error)?,
        TableColumn::new(
            "source_cell_row",
            TableColumnType::Scalar(TableScalarType::U64),
            false,
        )
        .map_err(map_declaration_error)?,
        TableColumn::new(
            "source_embedding_row",
            TableColumnType::Scalar(TableScalarType::U64),
            true,
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

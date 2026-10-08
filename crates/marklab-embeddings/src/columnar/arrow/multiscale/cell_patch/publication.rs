use std::{collections::BTreeMap, io};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactRef, ArtifactSchema, LocalArtifactStore,
};

use crate::{
    columnar::{
        multiscale::cell_patch_dependencies, MultiscaleColumnarPublicationError,
        SpatialColumnarWriteSummary,
    },
    multiscale::physical::{
        content_kind, schema_id, table_manifest, SpatialArtifactRole, SpatialPhysicalEncoding,
    },
    CellPatchLink, EmbeddingColumnarBudgets,
};

use super::writer::{write_cell_patch_assignment_table_arrow, write_cell_patch_edge_table_arrow};

/// Count/hash, replay, and publish a fresh canonical Arrow cell-patch assignment table.
pub fn publish_cell_patch_assignment_table_arrow(
    store: &LocalArtifactStore,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
    publish(
        store,
        link,
        budgets,
        SpatialArtifactRole::CellPatchAssignment,
        write_cell_patch_assignment_table_arrow,
    )
}

/// Count/hash, replay, and publish a fresh canonical Arrow cell-patch edge table.
pub fn publish_cell_patch_edge_table_arrow(
    store: &LocalArtifactStore,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
    publish(
        store,
        link,
        budgets,
        SpatialArtifactRole::CellPatchEdge,
        write_cell_patch_edge_table_arrow,
    )
}

fn publish(
    store: &LocalArtifactStore,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
    role: SpatialArtifactRole,
    write: fn(
        &mut dyn io::Write,
        &CellPatchLink,
        EmbeddingColumnarBudgets,
    ) -> Result<SpatialColumnarWriteSummary, crate::MultiscaleColumnarError>,
) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
    let summary = write(&mut io::sink(), link, budgets)?;
    let draft = draft(role, summary, link)?;
    store
        .publish_new_send(&draft, |output| {
            write(output, link, budgets)
                .map(|_| ())
                .map_err(|_| io::Error::other("canonical cell-patch Arrow replay failed"))
        })
        .map_err(MultiscaleColumnarPublicationError::Store)
}

fn draft(
    role: SpatialArtifactRole,
    summary: SpatialColumnarWriteSummary,
    link: &CellPatchLink,
) -> Result<ArtifactDraft, MultiscaleColumnarPublicationError> {
    let encoding = SpatialPhysicalEncoding::Arrow;
    let content = ArtifactRef::new(
        content_kind(role, encoding),
        summary.content_digest(),
        summary.encoded_byte_len(),
    )
    .map_err(map_declaration_error)?;
    let manifest =
        table_manifest(role, encoding, summary.row_count()).map_err(map_declaration_error)?;
    ArtifactDraft::new(
        content,
        ArtifactSchema::new(schema_id(role), 1).map_err(map_declaration_error)?,
        Some(manifest),
        cell_patch_dependencies(link).into(),
        BTreeMap::new(),
    )
    .map_err(map_declaration_error)
}

fn map_declaration_error<T>(_: T) -> MultiscaleColumnarPublicationError {
    MultiscaleColumnarPublicationError::Declaration
}

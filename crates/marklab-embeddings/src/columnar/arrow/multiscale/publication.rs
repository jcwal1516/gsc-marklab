use std::{collections::BTreeMap, io};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactRef, ArtifactSchema, ArtifactStoreError,
    LocalArtifactStore,
};
use thiserror::Error;

use crate::{ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph};

use super::writer::{write_patch_footprint_set_arrow, write_patch_overlap_graph_arrow};
use crate::{
    columnar::{EmbeddingColumnarBudgets, MultiscaleColumnarError},
    multiscale::physical::{
        content_kind, schema_id, table_manifest, SpatialArtifactRole, SpatialPhysicalEncoding,
    },
};

/// Failure while declaring, encoding, or publishing a C-05 spatial table.
#[derive(Debug, Error)]
pub enum MultiscaleColumnarPublicationError {
    /// Canonical encoding or resource validation failed before publication.
    #[error(transparent)]
    Columnar(#[from] MultiscaleColumnarError),
    /// A fixed artifact or table declaration could not be constructed.
    #[error("canonical spatial artifact declaration failed")]
    Declaration,
    /// Durable store publication, verification, or cleanup failed.
    #[error(transparent)]
    Store(#[from] ArtifactStoreError),
}

/// Count/hash, replay, and publish a fresh canonical Arrow footprint set.
pub fn publish_patch_footprint_set_arrow(
    store: &LocalArtifactStore,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
    let summary =
        write_patch_footprint_set_arrow(&mut io::sink(), expected, context, footprints, budgets)?;
    let role = SpatialArtifactRole::Footprint;
    let mut dependencies = vec![
        footprints.expected_patches_artifact_id(),
        footprints.patch_context_artifact_id(),
    ];
    dependencies.sort_unstable();
    let draft = draft(role, summary, dependencies)?;
    store
        .publish_new_send(&draft, |output| {
            write_patch_footprint_set_arrow(output, expected, context, footprints, budgets)
                .map(|_| ())
                .map_err(|_| io::Error::other("canonical footprint Arrow replay failed"))
        })
        .map_err(MultiscaleColumnarPublicationError::Store)
}

/// Count/hash, replay, and publish a fresh canonical Arrow overlap graph.
pub fn publish_patch_overlap_graph_arrow(
    store: &LocalArtifactStore,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
    let summary = write_patch_overlap_graph_arrow(
        &mut io::sink(),
        expected,
        context,
        footprints,
        overlap,
        budgets,
    )?;
    let role = SpatialArtifactRole::Overlap;
    let mut dependencies = vec![
        overlap.expected_patches_artifact_id(),
        footprints.patch_context_artifact_id(),
        overlap.patch_footprints_artifact_id(),
    ];
    dependencies.sort_unstable();
    let draft = draft(role, summary, dependencies)?;
    store
        .publish_new_send(&draft, |output| {
            write_patch_overlap_graph_arrow(output, expected, context, footprints, overlap, budgets)
                .map(|_| ())
                .map_err(|_| io::Error::other("canonical overlap Arrow replay failed"))
        })
        .map_err(MultiscaleColumnarPublicationError::Store)
}

fn draft(
    role: SpatialArtifactRole,
    summary: crate::columnar::SpatialColumnarWriteSummary,
    dependencies: Vec<marklab_project::ArtifactId>,
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
        dependencies,
        BTreeMap::new(),
    )
    .map_err(map_declaration_error)
}

fn map_declaration_error<T>(_: T) -> MultiscaleColumnarPublicationError {
    MultiscaleColumnarPublicationError::Declaration
}

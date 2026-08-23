use std::{collections::BTreeMap, io};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactRef, ArtifactSchema, LocalArtifactStore,
};

use crate::{ExpectedPatchSet, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph};

use super::writer::{write_patch_footprint_set_parquet, write_patch_overlap_graph_parquet};
use crate::{
    columnar::{
        EmbeddingColumnarBudgets, MultiscaleColumnarPublicationError, SpatialColumnarWriteSummary,
    },
    multiscale::physical::{
        content_kind, schema_id, table_manifest, SpatialArtifactRole, SpatialPhysicalEncoding,
    },
};

/// Count/hash, replay, and publish a fresh canonical Parquet footprint set.
pub fn publish_patch_footprint_set_parquet(
    store: &LocalArtifactStore,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
    let summary =
        write_patch_footprint_set_parquet(&mut io::sink(), expected, context, footprints, budgets)?;
    let mut dependencies = vec![
        footprints.expected_patches_artifact_id(),
        footprints.patch_context_artifact_id(),
    ];
    dependencies.sort_unstable();
    let draft = draft(SpatialArtifactRole::Footprint, summary, dependencies)?;
    store
        .publish_new_send(&draft, |output| {
            write_patch_footprint_set_parquet(output, expected, context, footprints, budgets)
                .map(|_| ())
                .map_err(|_| io::Error::other("canonical footprint Parquet replay failed"))
        })
        .map_err(MultiscaleColumnarPublicationError::Store)
}

/// Count/hash, replay, and publish a fresh canonical Parquet overlap graph.
pub fn publish_patch_overlap_graph_parquet(
    store: &LocalArtifactStore,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
    let summary = write_patch_overlap_graph_parquet(
        &mut io::sink(),
        expected,
        context,
        footprints,
        overlap,
        budgets,
    )?;
    let mut dependencies = vec![
        overlap.expected_patches_artifact_id(),
        footprints.patch_context_artifact_id(),
        overlap.patch_footprints_artifact_id(),
    ];
    dependencies.sort_unstable();
    let draft = draft(SpatialArtifactRole::Overlap, summary, dependencies)?;
    store
        .publish_new_send(&draft, |output| {
            write_patch_overlap_graph_parquet(
                output, expected, context, footprints, overlap, budgets,
            )
            .map(|_| ())
            .map_err(|_| io::Error::other("canonical overlap Parquet replay failed"))
        })
        .map_err(MultiscaleColumnarPublicationError::Store)
}

fn draft(
    role: SpatialArtifactRole,
    summary: SpatialColumnarWriteSummary,
    dependencies: Vec<marklab_project::ArtifactId>,
) -> Result<ArtifactDraft, MultiscaleColumnarPublicationError> {
    let encoding = SpatialPhysicalEncoding::Parquet;
    let content = ArtifactRef::new(
        content_kind(role, encoding),
        summary.content_digest(),
        summary.encoded_byte_len(),
    )
    .map_err(map_declaration_error)?;
    ArtifactDraft::new(
        content,
        ArtifactSchema::new(schema_id(role), 1).map_err(map_declaration_error)?,
        Some(table_manifest(role, encoding, summary.row_count()).map_err(map_declaration_error)?),
        dependencies,
        BTreeMap::new(),
    )
    .map_err(map_declaration_error)
}

fn map_declaration_error<T>(_: T) -> MultiscaleColumnarPublicationError {
    MultiscaleColumnarPublicationError::Declaration
}

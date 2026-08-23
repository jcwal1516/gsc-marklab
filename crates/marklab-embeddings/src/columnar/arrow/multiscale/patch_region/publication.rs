use std::{collections::BTreeMap, io};

use marklab_project::{
    ArtifactDraft, ArtifactPublication, ArtifactRef, ArtifactSchema, LocalArtifactStore,
};

use crate::{
    columnar::{MultiscaleColumnarPublicationError, SpatialColumnarWriteSummary},
    multiscale::physical::{content_kind, schema_id, table_manifest, SpatialPhysicalEncoding},
    EmbeddingColumnarBudgets, PatchRegionLink,
};

use super::{
    profile::{dependencies, role},
    writer::write_patch_region_link_arrow,
};

/// Count/hash, replay, and publish a fresh canonical Arrow patch-region link.
pub fn publish_patch_region_link_arrow(
    store: &LocalArtifactStore,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, MultiscaleColumnarPublicationError> {
    let summary = write_patch_region_link_arrow(&mut io::sink(), link, budgets)?;
    let draft = draft(summary, link)?;
    store
        .publish_new_send(&draft, |output| {
            write_patch_region_link_arrow(output, link, budgets)
                .map(|_| ())
                .map_err(|_| io::Error::other("canonical patch-region Arrow replay failed"))
        })
        .map_err(MultiscaleColumnarPublicationError::Store)
}

fn draft(
    summary: SpatialColumnarWriteSummary,
    link: &PatchRegionLink,
) -> Result<ArtifactDraft, MultiscaleColumnarPublicationError> {
    let role = role();
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
        dependencies(link).into(),
        BTreeMap::new(),
    )
    .map_err(map_declaration_error)
}

fn map_declaration_error<T>(_: T) -> MultiscaleColumnarPublicationError {
    MultiscaleColumnarPublicationError::Declaration
}

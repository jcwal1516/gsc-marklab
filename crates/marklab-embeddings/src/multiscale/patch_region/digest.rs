use marklab_project::ContentDigest;

use super::{types::PatchRegionDeclaration, validation::CommonBindings};
use crate::multiscale::{digest::LogicalDigest, error::MultiscaleEmbeddingError};

const RELATIONS_DOMAIN: &[u8] = b"marklab-patch-region-assessment-relations-v1";
const LINK_DOMAIN: &[u8] = b"marklab-patch-region-link-logical-v1";

pub(super) fn relations_digest(
    rows: &[PatchRegionDeclaration],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(RELATIONS_DOMAIN);
    digest.array_len(rows.len())?;
    frame_rows(&mut digest, rows);
    Ok(digest.finish())
}

pub(super) fn link_digest(
    bindings: &CommonBindings,
    assessment_artifact_id: marklab_project::ArtifactId,
    assessment_content_digest: ContentDigest,
    rows: &[PatchRegionDeclaration],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(LINK_DOMAIN);
    digest.artifact_id(bindings.expected_patches_artifact_id);
    digest.content_digest(bindings.expected_patches_logical_digest);
    digest.artifact_id(bindings.expected_regions_artifact_id);
    digest.content_digest(bindings.expected_regions_logical_digest);
    digest.artifact_id(bindings.patch_context_artifact_id);
    digest.content_digest(bindings.patch_context_logical_digest);
    digest.artifact_id(bindings.patch_footprints_artifact_id);
    digest.content_digest(bindings.patch_footprints_logical_digest);
    digest.artifact_id(bindings.converter_artifact_id);
    digest.content_digest(bindings.converter_content_digest);
    digest.artifact_id(assessment_artifact_id);
    digest.content_digest(assessment_content_digest);
    digest.text("expected_cartesian_exhaustive");
    digest.u64(bindings.assessed_pair_count);
    digest.array_len(rows.len())?;
    frame_rows(&mut digest, rows);
    Ok(digest.finish())
}

fn frame_rows(digest: &mut LogicalDigest, rows: &[PatchRegionDeclaration]) {
    for row in rows {
        digest.text(row.patch_id.as_str());
        digest.text(row.region_id.as_str());
        digest.text(row.relation.wire_name());
        digest.u64(row.numerator);
        digest.u64(row.denominator);
    }
}

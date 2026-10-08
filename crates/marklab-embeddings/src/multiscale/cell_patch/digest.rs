use marklab_project::ContentDigest;

use super::{
    types::{CellPatchAssignment, CellPatchAssignmentMode, CellPatchEdge},
    validation::CommonBindings,
};
use crate::multiscale::{digest::LogicalDigest, error::MultiscaleEmbeddingError};

const CELL_PATCH_DOMAIN: &[u8] = b"marklab-cell-patch-link-logical-v1";

pub(super) fn logical_digest(
    mode: CellPatchAssignmentMode,
    bindings: &CommonBindings,
    assignments: &[CellPatchAssignment],
    edges: &[CellPatchEdge],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(CELL_PATCH_DOMAIN);
    digest.text(mode.wire_name());
    digest.artifact_id(bindings.expected_cells_artifact_id);
    digest.content_digest(bindings.expected_cells_logical_digest);
    digest.artifact_id(bindings.expected_patches_artifact_id);
    digest.content_digest(bindings.expected_patches_logical_digest);
    digest.artifact_id(bindings.patch_context_artifact_id);
    digest.content_digest(bindings.patch_context_logical_digest);
    digest.artifact_id(bindings.patch_footprints_artifact_id);
    digest.content_digest(bindings.patch_footprints_logical_digest);
    digest.artifact_id(bindings.producer_artifact_id);
    digest.content_digest(bindings.producer_content_digest);
    digest.array_len(assignments.len())?;
    for assignment in assignments {
        digest.text(assignment.cell_id.as_str());
        digest.f64_bits(assignment.anchor_px[0].to_bits());
        digest.f64_bits(assignment.anchor_px[1].to_bits());
        digest.text(assignment.status.wire_name());
        digest.u64(assignment.edge_count);
        let start = usize::try_from(assignment.edge_start)
            .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
        let count = usize::try_from(assignment.edge_count)
            .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
        let end = start
            .checked_add(count)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        for edge in edges
            .get(start..end)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?
        {
            digest.text(edge.patch_id.as_str());
            if let Some(weight) = edge.weight {
                digest.text("weight_present");
                digest.u64(weight.numerator());
                digest.u64(weight.denominator());
            } else {
                digest.text("weight_absent");
            }
        }
    }
    Ok(digest.finish())
}

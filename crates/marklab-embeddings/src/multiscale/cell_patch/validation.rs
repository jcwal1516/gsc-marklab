use marklab_data::{CohortHierarchy, HierarchyId, HierarchyKind, SlideId};
use marklab_project::{ArtifactId, ContentDigest};

use super::types::{CellPatchAnchor, CellPatchLinkBindings, DeclaredCellPatchAssignment};
use crate::{
    multiscale::{
        context::PatchEmbeddingContext, error::MultiscaleEmbeddingError,
        expected::ExpectedPatchSet, footprint::PatchFootprintSet,
    },
    ExpectedCellSet,
};

pub(super) struct CommonBindings {
    pub(super) owning_slide_id: SlideId,
    pub(super) expected_cells_artifact_id: ArtifactId,
    pub(super) expected_cells_logical_digest: ContentDigest,
    pub(super) expected_patches_artifact_id: ArtifactId,
    pub(super) expected_patches_logical_digest: ContentDigest,
    pub(super) patch_context_artifact_id: ArtifactId,
    pub(super) patch_context_logical_digest: ContentDigest,
    pub(super) patch_footprints_artifact_id: ArtifactId,
    pub(super) patch_footprints_logical_digest: ContentDigest,
    pub(super) producer_artifact_id: ArtifactId,
    pub(super) producer_content_digest: ContentDigest,
}

pub(super) fn validate_common(
    hierarchy: &CohortHierarchy,
    expected_cells: &ExpectedCellSet,
    expected_patches: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    bindings: &CellPatchLinkBindings,
) -> Result<CommonBindings, MultiscaleEmbeddingError> {
    if expected_cells.cells().len() > super::resources::MAX_ASSIGNMENTS {
        return Err(MultiscaleEmbeddingError::RowCountExceeded {
            observed: expected_cells.cells().len(),
            maximum: super::resources::MAX_ASSIGNMENTS,
        });
    }
    if bindings.anchor_frame_id() != context.image_frame_id() {
        return Err(MultiscaleEmbeddingError::CellPatchFrameMismatch);
    }
    if expected_patches.owning_slide_id() != context.owning_slide_id()
        || expected_patches.logical_digest() != footprints.expected_patches_logical_digest()
        || context.logical_digest() != footprints.patch_context_logical_digest()
        || expected_patches.ids().len() != footprints.footprints().len()
        || expected_patches
            .ids()
            .iter()
            .zip(footprints.footprints())
            .any(|(expected, footprint)| expected != footprint.patch_id())
    {
        return Err(MultiscaleEmbeddingError::CellPatchInputMismatch);
    }
    validate_cell_hierarchy(hierarchy, expected_cells, context.owning_slide_id())?;
    let dependencies = [
        bindings.expected_cells_artifact_id,
        footprints.expected_patches_artifact_id(),
        footprints.patch_context_artifact_id(),
        bindings.patch_footprints_artifact_id,
        bindings.producer_artifact_id,
    ];
    let mut sorted = dependencies;
    sorted.sort_unstable();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(MultiscaleEmbeddingError::DuplicateCellPatchArtifactDependency);
    }
    Ok(CommonBindings {
        owning_slide_id: context.owning_slide_id().clone(),
        expected_cells_artifact_id: bindings.expected_cells_artifact_id,
        expected_cells_logical_digest: expected_cells.logical_digest(),
        expected_patches_artifact_id: footprints.expected_patches_artifact_id(),
        expected_patches_logical_digest: footprints.expected_patches_logical_digest(),
        patch_context_artifact_id: footprints.patch_context_artifact_id(),
        patch_context_logical_digest: footprints.patch_context_logical_digest(),
        patch_footprints_artifact_id: bindings.patch_footprints_artifact_id,
        patch_footprints_logical_digest: footprints.logical_digest(),
        producer_artifact_id: bindings.producer_artifact_id,
        producer_content_digest: bindings.producer_content_digest,
    })
}

pub(super) fn validate_anchor_order(
    expected: &ExpectedCellSet,
    anchors: &[CellPatchAnchor],
) -> Result<(), MultiscaleEmbeddingError> {
    if anchors.len() != expected.cells().len()
        || anchors
            .iter()
            .zip(expected.cells())
            .any(|(anchor, expected)| &anchor.cell_id != expected)
    {
        return Err(MultiscaleEmbeddingError::CellPatchSetMismatch);
    }
    if anchors
        .iter()
        .any(|anchor| anchor.anchor_px.iter().any(|value| !value.is_finite()))
    {
        return Err(MultiscaleEmbeddingError::InvalidCellPatchAnchor);
    }
    Ok(())
}

pub(super) fn validate_declaration_order(
    expected: &ExpectedCellSet,
    declarations: &[DeclaredCellPatchAssignment],
) -> Result<(), MultiscaleEmbeddingError> {
    if declarations.len() != expected.cells().len()
        || declarations
            .iter()
            .zip(expected.cells())
            .any(|(declaration, expected)| &declaration.anchor.cell_id != expected)
    {
        return Err(MultiscaleEmbeddingError::CellPatchSetMismatch);
    }
    if declarations.iter().any(|declaration| {
        declaration
            .anchor
            .anchor_px
            .iter()
            .any(|value| !value.is_finite())
    }) {
        return Err(MultiscaleEmbeddingError::InvalidCellPatchAnchor);
    }
    Ok(())
}

fn validate_cell_hierarchy(
    hierarchy: &CohortHierarchy,
    expected: &ExpectedCellSet,
    owning_slide: &SlideId,
) -> Result<(), MultiscaleEmbeddingError> {
    let owning = HierarchyId::from(owning_slide.clone());
    for (row, cell_id) in expected.cells().iter().enumerate() {
        let mut current = HierarchyId::from(cell_id.clone());
        if !hierarchy.contains(&current) {
            return Err(MultiscaleEmbeddingError::CellPatchHierarchyMismatch { row });
        }
        loop {
            if current.kind() == HierarchyKind::Slide {
                if current != owning {
                    return Err(MultiscaleEmbeddingError::CellPatchHierarchyMismatch { row });
                }
                break;
            }
            current = hierarchy
                .parent(&current)
                .cloned()
                .ok_or(MultiscaleEmbeddingError::CellPatchHierarchyMismatch { row })?;
        }
    }
    Ok(())
}

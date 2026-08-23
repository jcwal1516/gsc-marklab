mod contained;
mod digest;
mod interpolation;
mod resources;
mod types;
mod validation;

use std::fmt;

use marklab_data::{CohortHierarchy, SlideId};
use marklab_project::{ArtifactId, ContentDigest};

use self::{
    resources::{
        anchor_input_bytes, declaration_input_bytes, enforce_retained, enforce_working,
        output_bytes_from_counts, retained_bytes,
    },
    validation::{
        validate_anchor_order, validate_common, validate_declaration_order, CommonBindings,
    },
};
use super::{
    context::PatchEmbeddingContext, error::MultiscaleEmbeddingError, expected::ExpectedPatchSet,
    footprint::PatchFootprintSet,
};
use crate::ExpectedCellSet;

pub use types::{
    CellPatchAnchor, CellPatchAssignment, CellPatchAssignmentMode, CellPatchAssignmentStatus,
    CellPatchContributor, CellPatchEdge, CellPatchLinkBindings, CellPatchWeight,
    DeclaredCellPatchAssignment,
};

/// Immutable grouped cell assignments plus vector-free canonical patch edges.
#[derive(Clone, PartialEq)]
pub struct CellPatchLink {
    mode: CellPatchAssignmentMode,
    owning_slide_id: SlideId,
    assignments: Box<[CellPatchAssignment]>,
    edges: Box<[CellPatchEdge]>,
    expected_cells_artifact_id: ArtifactId,
    expected_cells_logical_digest: ContentDigest,
    expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    patch_context_artifact_id: ArtifactId,
    patch_context_logical_digest: ContentDigest,
    patch_footprints_artifact_id: ArtifactId,
    patch_footprints_logical_digest: ContentDigest,
    producer_artifact_id: ArtifactId,
    producer_content_digest: ContentDigest,
    logical_digest: ContentDigest,
}

impl fmt::Debug for CellPatchLink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellPatchLink")
            .field("mode", &self.mode)
            .field("assignment_count", &self.assignments.len())
            .field("edge_count", &self.edges.len())
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl CellPatchLink {
    /// Derive all positive half-open containment edges with two identical indexed passes.
    ///
    /// Runtime is `O(p log p + c log p + k + e log e)`, where `k` is the number of
    /// footprint candidates in the four neighboring buckets. Degenerate same-bucket layouts can
    /// make `k` approach `c * p`; `maximum_candidate_checks` closes that CPU-resource boundary
    /// independently on both deterministic passes. Retained output remains `O(c + e)` and working
    /// storage adds `O(p)`. C-05's scale gate must characterize the cliff before a more complex
    /// index is justified.
    #[allow(clippy::too_many_arguments)]
    pub fn derive_contained_shared(
        hierarchy: &CohortHierarchy,
        expected_cells: &ExpectedCellSet,
        expected_patches: &ExpectedPatchSet,
        context: &PatchEmbeddingContext,
        footprints: &PatchFootprintSet,
        bindings: &CellPatchLinkBindings,
        anchors: Vec<CellPatchAnchor>,
        maximum_candidate_checks: usize,
        maximum_retained_bytes: usize,
        maximum_working_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let common = validate_common(
            hierarchy,
            expected_cells,
            expected_patches,
            context,
            footprints,
            bindings,
        )?;
        validate_anchor_order(expected_cells, &anchors)?;
        let input_bytes = anchor_input_bytes(&anchors)?;
        let scratch_bytes = contained::scratch_bytes(footprints.footprints().len())?;
        let first_peak = input_bytes
            .checked_add(scratch_bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        enforce_working(first_peak, maximum_working_bytes)?;

        let counted = contained::run(
            &anchors,
            context,
            footprints,
            maximum_candidate_checks,
            None,
        )?;
        let output_bytes = output_bytes_from_counts(
            common.owning_slide_id.as_str().len(),
            anchors.len(),
            counted.cell_text_bytes,
            counted.edge_count,
            counted.edge_text_bytes,
        )?;
        enforce_retained(output_bytes, maximum_retained_bytes)?;
        let peak = first_peak
            .checked_add(output_bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        enforce_working(peak, maximum_working_bytes)?;

        let materialized = contained::run(
            &anchors,
            context,
            footprints,
            maximum_candidate_checks,
            Some(counted.edge_count),
        )?;
        verify_pass_counts(
            (
                counted.edge_count,
                counted.edge_text_bytes,
                counted.cell_text_bytes,
                counted.candidate_checks,
            ),
            (
                materialized.edge_count,
                materialized.edge_text_bytes,
                materialized.cell_text_bytes,
                materialized.candidate_checks,
            ),
        )?;
        finish(
            CellPatchAssignmentMode::ContainedShared,
            common,
            materialized
                .assignments
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
            materialized
                .edges
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
            output_bytes,
        )
    }

    /// Validate and preserve exact producer-declared weighted interpolation groups.
    #[allow(clippy::too_many_arguments)]
    pub fn from_declared_weighted_interpolation(
        hierarchy: &CohortHierarchy,
        expected_cells: &ExpectedCellSet,
        expected_patches: &ExpectedPatchSet,
        context: &PatchEmbeddingContext,
        footprints: &PatchFootprintSet,
        bindings: &CellPatchLinkBindings,
        declarations: Vec<DeclaredCellPatchAssignment>,
        maximum_retained_bytes: usize,
        maximum_working_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let common = validate_common(
            hierarchy,
            expected_cells,
            expected_patches,
            context,
            footprints,
            bindings,
        )?;
        validate_declaration_order(expected_cells, &declarations)?;
        let input_bytes = declaration_input_bytes(&declarations)?;
        enforce_working(input_bytes, maximum_working_bytes)?;
        let counted = interpolation::run(&declarations, expected_patches, None)?;
        let output_bytes = output_bytes_from_counts(
            common.owning_slide_id.as_str().len(),
            declarations.len(),
            counted.cell_text_bytes,
            counted.edge_count,
            counted.edge_text_bytes,
        )?;
        enforce_retained(output_bytes, maximum_retained_bytes)?;
        let peak = input_bytes
            .checked_add(output_bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        enforce_working(peak, maximum_working_bytes)?;
        let materialized =
            interpolation::run(&declarations, expected_patches, Some(counted.edge_count))?;
        verify_pass_counts(
            (
                counted.edge_count,
                counted.edge_text_bytes,
                counted.cell_text_bytes,
                0,
            ),
            (
                materialized.edge_count,
                materialized.edge_text_bytes,
                materialized.cell_text_bytes,
                0,
            ),
        )?;
        finish(
            CellPatchAssignmentMode::DeclaredWeightedInterpolation,
            common,
            materialized
                .assignments
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
            materialized
                .edges
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
            output_bytes,
        )
    }

    /// Closed table-wide assignment mode.
    pub fn mode(&self) -> CellPatchAssignmentMode {
        self.mode
    }

    /// Owning slide shared by cells, patches, context, and footprints.
    pub fn owning_slide_id(&self) -> &SlideId {
        &self.owning_slide_id
    }

    /// Number of expected-cell assignment groups.
    pub fn assignment_count(&self) -> usize {
        self.assignments.len()
    }

    /// Canonical assignment rows in expected-cell order.
    pub fn assignments(&self) -> &[CellPatchAssignment] {
        &self.assignments
    }

    /// Number of vector-free cell-to-patch edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Canonical edges ordered by assignment row then patch identity.
    pub fn edges(&self) -> &[CellPatchEdge] {
        &self.edges
    }

    /// Borrow one assignment's exact contiguous edge slice.
    pub fn edges_for_assignment(
        &self,
        assignment_row: usize,
    ) -> Result<&[CellPatchEdge], MultiscaleEmbeddingError> {
        let assignment = self.assignments.get(assignment_row).ok_or(
            MultiscaleEmbeddingError::RowOutOfBounds {
                index: assignment_row,
                row_count: self.assignments.len(),
            },
        )?;
        let start = usize::try_from(assignment.edge_start)
            .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
        let count = usize::try_from(assignment.edge_count)
            .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
        let end = start
            .checked_add(count)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        self.edges
            .get(start..end)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    }

    /// Expected-cell artifact identity.
    pub fn expected_cells_artifact_id(&self) -> ArtifactId {
        self.expected_cells_artifact_id
    }

    /// Exact expected-cell logical identity.
    pub fn expected_cells_logical_digest(&self) -> ContentDigest {
        self.expected_cells_logical_digest
    }

    /// Expected-patch artifact identity.
    pub fn expected_patches_artifact_id(&self) -> ArtifactId {
        self.expected_patches_artifact_id
    }

    /// Exact expected-patch logical identity.
    pub fn expected_patches_logical_digest(&self) -> ContentDigest {
        self.expected_patches_logical_digest
    }

    /// Patch-context artifact identity.
    pub fn patch_context_artifact_id(&self) -> ArtifactId {
        self.patch_context_artifact_id
    }

    /// Exact patch-context logical identity.
    pub fn patch_context_logical_digest(&self) -> ContentDigest {
        self.patch_context_logical_digest
    }

    /// Patch-footprint artifact identity.
    pub fn patch_footprints_artifact_id(&self) -> ArtifactId {
        self.patch_footprints_artifact_id
    }

    /// Exact patch-footprint logical identity.
    pub fn patch_footprints_logical_digest(&self) -> ContentDigest {
        self.patch_footprints_logical_digest
    }

    /// Producer artifact identity.
    pub fn producer_artifact_id(&self) -> ArtifactId {
        self.producer_artifact_id
    }

    /// Producer exact content digest.
    pub fn producer_content_digest(&self) -> ContentDigest {
        self.producer_content_digest
    }

    /// Domain-separated logical identity over bindings, groups, anchors, and edges.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }
}

fn finish(
    mode: CellPatchAssignmentMode,
    common: CommonBindings,
    assignments: Box<[CellPatchAssignment]>,
    edges: Box<[CellPatchEdge]>,
    expected_retained_bytes: usize,
) -> Result<CellPatchLink, MultiscaleEmbeddingError> {
    if retained_bytes(common.owning_slide_id.as_str().len(), &assignments, &edges)?
        != expected_retained_bytes
    {
        return Err(MultiscaleEmbeddingError::SizeOverflow);
    }
    let logical_digest = digest::logical_digest(mode, &common, &assignments, &edges)?;
    Ok(CellPatchLink {
        mode,
        owning_slide_id: common.owning_slide_id,
        assignments,
        edges,
        expected_cells_artifact_id: common.expected_cells_artifact_id,
        expected_cells_logical_digest: common.expected_cells_logical_digest,
        expected_patches_artifact_id: common.expected_patches_artifact_id,
        expected_patches_logical_digest: common.expected_patches_logical_digest,
        patch_context_artifact_id: common.patch_context_artifact_id,
        patch_context_logical_digest: common.patch_context_logical_digest,
        patch_footprints_artifact_id: common.patch_footprints_artifact_id,
        patch_footprints_logical_digest: common.patch_footprints_logical_digest,
        producer_artifact_id: common.producer_artifact_id,
        producer_content_digest: common.producer_content_digest,
        logical_digest,
    })
}

fn verify_pass_counts(
    expected: (usize, usize, usize, usize),
    observed: (usize, usize, usize, usize),
) -> Result<(), MultiscaleEmbeddingError> {
    if expected != observed {
        return Err(MultiscaleEmbeddingError::SizeOverflow);
    }
    Ok(())
}

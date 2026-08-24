use std::fmt;

use marklab_project::{ArtifactId, ContentDigest};

use super::CellPatchAssignmentMode;
#[cfg(feature = "parquet")]
use super::{
    records::{
        VerifiedCellPatchInputArtifactGraph, VerifiedDirectPatchEmbeddingArtifactGraph,
        VerifiedPatchRegionInputArtifactGraph,
    },
    CellPatchLink, PatchEmbeddingContext, PatchFootprintSet, PatchOverlapGraph, PatchRegionLink,
};

/// Runtime-only proof of one fully decoded physical patch-footprint artifact.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedPatchFootprintArtifact {
    pub(crate) artifact_id: ArtifactId,
    pub(crate) expected_patches_artifact_id: ArtifactId,
    pub(crate) patch_context_artifact_id: ArtifactId,
    pub(crate) logical_digest: ContentDigest,
    pub(crate) row_count: u64,
}

impl fmt::Debug for VerifiedPatchFootprintArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedPatchFootprintArtifact")
            .field("row_count", &self.row_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedPatchFootprintArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        artifact_id: ArtifactId,
        context: &PatchEmbeddingContext,
        footprints: &PatchFootprintSet,
        graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        if artifact_id != graph.patch_footprints_artifact_id
            || footprints.expected_patches_artifact_id() != graph.expected_patches_artifact_id
            || footprints.expected_patches_logical_digest() != graph.expected_patches_logical_digest
            || footprints.patch_context_artifact_id() != graph.patch_context_artifact_id
            || context.logical_digest() != graph.patch_context_logical_digest
            || footprints.logical_digest() != graph.patch_footprints_logical_digest
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            artifact_id,
            expected_patches_artifact_id: graph.expected_patches_artifact_id,
            patch_context_artifact_id: graph.patch_context_artifact_id,
            logical_digest: footprints.logical_digest(),
            row_count: u64::try_from(footprints.row_count())
                .map_err(|_| crate::columnar::MultiscaleColumnarError::SizeOverflow)?,
        })
    }

    /// Exact physical footprint artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent logical footprint identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    /// Exact validated footprint row count.
    pub fn row_count(self) -> u64 {
        self.row_count
    }
}

/// Runtime-only proof of one fully decoded physical patch-overlap artifact.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedPatchOverlapArtifact {
    pub(crate) artifact_id: ArtifactId,
    pub(crate) expected_patches_artifact_id: ArtifactId,
    pub(crate) patch_context_artifact_id: ArtifactId,
    pub(crate) footprint_artifact_id: ArtifactId,
    pub(crate) logical_digest: ContentDigest,
    pub(crate) row_count: u64,
}

impl fmt::Debug for VerifiedPatchOverlapArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedPatchOverlapArtifact")
            .field("row_count", &self.row_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedPatchOverlapArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        artifact_id: ArtifactId,
        footprints: &PatchFootprintSet,
        overlap: &PatchOverlapGraph,
        verified_footprints: VerifiedPatchFootprintArtifact,
        graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        if artifact_id != graph.patch_overlap_artifact_id
            || overlap.logical_digest() != graph.patch_overlap_logical_digest
            || overlap.expected_patches_artifact_id() != graph.expected_patches_artifact_id
            || overlap.expected_patches_logical_digest() != graph.expected_patches_logical_digest
            || overlap.patch_footprints_artifact_id() != graph.patch_footprints_artifact_id
            || overlap.patch_footprints_logical_digest() != graph.patch_footprints_logical_digest
            || footprints.logical_digest() != graph.patch_footprints_logical_digest
            || verified_footprints.artifact_id != graph.patch_footprints_artifact_id
            || verified_footprints.expected_patches_artifact_id
                != graph.expected_patches_artifact_id
            || verified_footprints.patch_context_artifact_id != graph.patch_context_artifact_id
            || verified_footprints.logical_digest != graph.patch_footprints_logical_digest
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            artifact_id,
            expected_patches_artifact_id: graph.expected_patches_artifact_id,
            patch_context_artifact_id: graph.patch_context_artifact_id,
            footprint_artifact_id: graph.patch_footprints_artifact_id,
            logical_digest: overlap.logical_digest(),
            row_count: u64::try_from(overlap.edge_count())
                .map_err(|_| crate::columnar::MultiscaleColumnarError::SizeOverflow)?,
        })
    }

    /// Exact physical overlap artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent logical overlap identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    /// Exact validated overlap-edge row count.
    pub fn row_count(self) -> u64 {
        self.row_count
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct VerifiedCellPatchBindings {
    mode: CellPatchAssignmentMode,
    expected_cells_artifact_id: ArtifactId,
    expected_patches_artifact_id: ArtifactId,
    patch_context_artifact_id: ArtifactId,
    patch_footprints_artifact_id: ArtifactId,
    producer_artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    assignment_count: u64,
    edge_count: u64,
}

impl VerifiedCellPatchBindings {
    #[cfg(feature = "parquet")]
    fn new(
        link: &CellPatchLink,
        graph: VerifiedCellPatchInputArtifactGraph,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        let assignment_count = u64::try_from(link.assignment_count())
            .map_err(|_| crate::columnar::MultiscaleColumnarError::SizeOverflow)?;
        let edge_count = u64::try_from(link.edge_count())
            .map_err(|_| crate::columnar::MultiscaleColumnarError::SizeOverflow)?;
        if graph.mode != link.mode()
            || graph.expected_cells_artifact_id != link.expected_cells_artifact_id()
            || graph.expected_patches_artifact_id != link.expected_patches_artifact_id()
            || graph.patch_context_artifact_id != link.patch_context_artifact_id()
            || graph.patch_footprints_artifact_id != link.patch_footprints_artifact_id()
            || graph.producer_artifact_id != link.producer_artifact_id()
            || graph.link_logical_digest != link.logical_digest()
            || graph.assignment_count != assignment_count
            || graph.edge_count != edge_count
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            mode: graph.mode,
            expected_cells_artifact_id: graph.expected_cells_artifact_id,
            expected_patches_artifact_id: graph.expected_patches_artifact_id,
            patch_context_artifact_id: graph.patch_context_artifact_id,
            patch_footprints_artifact_id: graph.patch_footprints_artifact_id,
            producer_artifact_id: graph.producer_artifact_id,
            logical_digest: graph.link_logical_digest,
            assignment_count,
            edge_count,
        })
    }

    #[cfg(feature = "parquet")]
    fn matches(self, other: Self) -> bool {
        self.mode == other.mode
            && self.expected_cells_artifact_id == other.expected_cells_artifact_id
            && self.expected_patches_artifact_id == other.expected_patches_artifact_id
            && self.patch_context_artifact_id == other.patch_context_artifact_id
            && self.patch_footprints_artifact_id == other.patch_footprints_artifact_id
            && self.producer_artifact_id == other.producer_artifact_id
            && self.logical_digest == other.logical_digest
            && self.assignment_count == other.assignment_count
            && self.edge_count == other.edge_count
    }
}

/// Runtime-only proof of one fully decoded physical cell-patch assignment artifact.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedCellPatchAssignmentArtifact {
    artifact_id: ArtifactId,
    bindings: VerifiedCellPatchBindings,
}

impl fmt::Debug for VerifiedCellPatchAssignmentArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCellPatchAssignmentArtifact")
            .field("mode", &self.bindings.mode)
            .field("row_count", &self.bindings.assignment_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedCellPatchAssignmentArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        artifact_id: ArtifactId,
        link: &CellPatchLink,
        graph: VerifiedCellPatchInputArtifactGraph,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        Ok(Self {
            artifact_id,
            bindings: VerifiedCellPatchBindings::new(link, graph)?,
        })
    }

    /// Exact physical assignment artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent cell-patch logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.bindings.logical_digest
    }

    /// Exact validated assignment row count.
    pub fn row_count(self) -> u64 {
        self.bindings.assignment_count
    }
}

/// Runtime-only proof of one fully decoded physical cell-patch edge artifact.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedCellPatchEdgeArtifact {
    artifact_id: ArtifactId,
    bindings: VerifiedCellPatchBindings,
}

impl fmt::Debug for VerifiedCellPatchEdgeArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCellPatchEdgeArtifact")
            .field("mode", &self.bindings.mode)
            .field("row_count", &self.bindings.edge_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedCellPatchEdgeArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        artifact_id: ArtifactId,
        link: &CellPatchLink,
        graph: VerifiedCellPatchInputArtifactGraph,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        Ok(Self {
            artifact_id,
            bindings: VerifiedCellPatchBindings::new(link, graph)?,
        })
    }

    /// Exact physical edge artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent cell-patch logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.bindings.logical_digest
    }

    /// Exact validated edge row count.
    pub fn row_count(self) -> u64 {
        self.bindings.edge_count
    }
}

/// Runtime-only proof that independently decoded assignment and edge artifacts form one link.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedCellPatchLinkArtifact {
    assignment_artifact_id: ArtifactId,
    edge_artifact_id: ArtifactId,
    bindings: VerifiedCellPatchBindings,
}

impl fmt::Debug for VerifiedCellPatchLinkArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCellPatchLinkArtifact")
            .field("mode", &self.bindings.mode)
            .field("assignment_count", &self.bindings.assignment_count)
            .field("edge_count", &self.bindings.edge_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedCellPatchLinkArtifact {
    /// Pair independently verified physical halves regardless of their Arrow/Parquet formats.
    ///
    /// # Errors
    ///
    /// Returns an artifact-binding error when the receipts prove different logical links or when
    /// the two role-specific receipts unexpectedly name one physical artifact.
    #[cfg(feature = "parquet")]
    pub fn from_verified_halves(
        assignment: VerifiedCellPatchAssignmentArtifact,
        edge: VerifiedCellPatchEdgeArtifact,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        if assignment.artifact_id == edge.artifact_id || !assignment.bindings.matches(edge.bindings)
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            assignment_artifact_id: assignment.artifact_id,
            edge_artifact_id: edge.artifact_id,
            bindings: assignment.bindings,
        })
    }

    /// Exact physical assignment artifact identity.
    pub fn assignment_artifact_id(self) -> ArtifactId {
        self.assignment_artifact_id
    }

    /// Exact physical edge artifact identity.
    pub fn edge_artifact_id(self) -> ArtifactId {
        self.edge_artifact_id
    }

    /// Format-independent cell-patch logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.bindings.logical_digest
    }

    /// Exact validated assignment row count.
    pub fn assignment_count(self) -> u64 {
        self.bindings.assignment_count
    }

    /// Exact validated edge row count.
    pub fn edge_count(self) -> u64 {
        self.bindings.edge_count
    }
}

/// Runtime-only proof of one fully decoded producer-declared exhaustive patch-region link.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedPatchRegionLinkArtifact {
    artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    assessed_pair_count: u64,
    nonzero_relation_count: u64,
}

impl fmt::Debug for VerifiedPatchRegionLinkArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedPatchRegionLinkArtifact")
            .field("assessed_pair_count", &self.assessed_pair_count)
            .field("nonzero_relation_count", &self.nonzero_relation_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedPatchRegionLinkArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        artifact_id: ArtifactId,
        link: &PatchRegionLink,
        graph: VerifiedPatchRegionInputArtifactGraph,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        let nonzero_relation_count = u64::try_from(link.nonzero_relation_count())
            .map_err(|_| crate::columnar::MultiscaleColumnarError::SizeOverflow)?;
        if graph.expected_patches_artifact_id != link.expected_patches_artifact_id()
            || graph.expected_regions_artifact_id != link.expected_regions_artifact_id()
            || graph.patch_context_artifact_id != link.patch_context_artifact_id()
            || graph.patch_footprints_artifact_id != link.patch_footprints_artifact_id()
            || graph.converter_artifact_id != link.converter_artifact_id()
            || graph.converter_content_digest != link.converter_content_digest()
            || graph.assessment_artifact_id != link.assessment_artifact_id()
            || graph.assessment_content_digest != link.assessment_content_digest()
            || graph.link_logical_digest != link.logical_digest()
            || graph.assessed_pair_count != link.assessed_pair_count()
            || graph.nonzero_relation_count != nonzero_relation_count
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            artifact_id,
            logical_digest: graph.link_logical_digest,
            assessed_pair_count: graph.assessed_pair_count,
            nonzero_relation_count,
        })
    }

    /// Exact physical patch-region artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent patch-region link identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    /// Exact producer-declared Cartesian pair count.
    pub fn assessed_pair_count(self) -> u64 {
        self.assessed_pair_count
    }

    /// Exact stored nonzero relation row count.
    pub fn row_count(self) -> u64 {
        self.nonzero_relation_count
    }
}

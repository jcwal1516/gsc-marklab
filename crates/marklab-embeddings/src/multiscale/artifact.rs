use std::fmt;

use marklab_project::{ArtifactId, ContentDigest};

#[cfg(feature = "parquet")]
use super::{
    records::VerifiedDirectPatchEmbeddingArtifactGraph, PatchEmbeddingContext, PatchFootprintSet,
    PatchOverlapGraph,
};

/// Runtime-only proof of one fully decoded physical patch-footprint artifact.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedPatchFootprintArtifact {
    artifact_id: ArtifactId,
    expected_patches_artifact_id: ArtifactId,
    patch_context_artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    row_count: u64,
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
    artifact_id: ArtifactId,
    expected_patches_artifact_id: ArtifactId,
    patch_context_artifact_id: ArtifactId,
    footprint_artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    row_count: u64,
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

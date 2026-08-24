use std::fmt;

use marklab_project::{ArtifactId, ContentDigest};

use super::MultiscaleEmbeddingQcSummary;
#[cfg(feature = "parquet")]
use super::{
    PatchEmbeddingSourceRowLink, PatchEmbeddingTable, VerifiedDirectPatchEmbeddingArtifactGraph,
    VerifiedPatchFootprintArtifact, VerifiedPatchOverlapArtifact,
};

/// Runtime-only proof that one patch-support descriptor has fully decoded footprint and overlap
/// artifacts matching its exact structural graph.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedPatchEmbeddingSupportArtifact {
    artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    expected_patches_artifact_id: ArtifactId,
    patch_context_artifact_id: ArtifactId,
    patch_footprints_artifact_id: ArtifactId,
    patch_overlap_artifact_id: ArtifactId,
    row_count: u64,
}

impl fmt::Debug for VerifiedPatchEmbeddingSupportArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedPatchEmbeddingSupportArtifact")
            .field("row_count", &self.row_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedPatchEmbeddingSupportArtifact {
    /// Compose exact structural, footprint, and overlap proofs without imposing one physical
    /// format on both table artifacts.
    ///
    /// # Errors
    ///
    /// Returns an artifact-binding error when either physical receipt belongs to a different
    /// direct-patch graph or when the receipts do not form one footprint/overlap chain.
    #[cfg(feature = "parquet")]
    pub fn from_verified_components(
        graph: VerifiedDirectPatchEmbeddingArtifactGraph,
        footprints: VerifiedPatchFootprintArtifact,
        overlap: VerifiedPatchOverlapArtifact,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        if footprints.artifact_id != graph.patch_footprints_artifact_id
            || footprints.expected_patches_artifact_id != graph.expected_patches_artifact_id
            || footprints.patch_context_artifact_id != graph.patch_context_artifact_id
            || footprints.logical_digest != graph.patch_footprints_logical_digest
            || overlap.artifact_id != graph.patch_overlap_artifact_id
            || overlap.expected_patches_artifact_id != graph.expected_patches_artifact_id
            || overlap.patch_context_artifact_id != graph.patch_context_artifact_id
            || overlap.footprint_artifact_id != graph.patch_footprints_artifact_id
            || overlap.logical_digest != graph.patch_overlap_logical_digest
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            artifact_id: graph.patch_support_artifact_id,
            logical_digest: graph.patch_support_logical_digest,
            expected_patches_artifact_id: graph.expected_patches_artifact_id,
            patch_context_artifact_id: graph.patch_context_artifact_id,
            patch_footprints_artifact_id: footprints.artifact_id,
            patch_overlap_artifact_id: overlap.artifact_id,
            row_count: footprints.row_count,
        })
    }

    /// Exact patch-support artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent patch-support logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    /// Number of expected patch rows covered by the verified support chain.
    pub fn row_count(self) -> u64 {
        self.row_count
    }
}

/// Runtime-only proof of one fully decoded direct-patch embedding table.
///
/// This receipt proves the exact expected set, support, provenance, source-row statuses, physical
/// profile, logical digest, and QC summary. It does not prove the semantics or component-wise
/// correspondence of opaque source-vector bytes.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedPatchEmbeddingTableArtifact {
    artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    qc_summary: MultiscaleEmbeddingQcSummary,
    expected_patches_artifact_id: ArtifactId,
    support_artifact_id: ArtifactId,
    provenance_artifact_id: ArtifactId,
    source_row_link_artifact_id: ArtifactId,
    dimension: u32,
}

impl fmt::Debug for VerifiedPatchEmbeddingTableArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedPatchEmbeddingTableArtifact")
            .field("row_count", &self.qc_summary.row_count())
            .field("dimension", &self.dimension)
            .finish_non_exhaustive()
    }
}

impl VerifiedPatchEmbeddingTableArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        artifact_id: ArtifactId,
        table: &PatchEmbeddingTable,
        source_row_link: &PatchEmbeddingSourceRowLink,
        support: VerifiedPatchEmbeddingSupportArtifact,
        graph: VerifiedDirectPatchEmbeddingArtifactGraph,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        let row_count = u64::try_from(table.row_count())
            .map_err(|_| crate::columnar::MultiscaleColumnarError::SizeOverflow)?;
        if source_row_link.expected_patches_artifact_id() != graph.expected_patches_artifact_id
            || source_row_link.expected_patches_logical_digest()
                != graph.expected_patches_logical_digest
            || source_row_link.logical_digest() != graph.source_row_link_logical_digest
            || source_row_link.entries().len() != table.row_count()
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        for (index, source) in source_row_link.entries().iter().enumerate() {
            let row = table
                .row(index)
                .map_err(|_| crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch)?;
            if row.patch_id() != source.patch_id() || row.status() != source.status() {
                return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
            }
        }
        if artifact_id == table.expected_entities_artifact_id()
            || artifact_id == table.support_artifact_id()
            || artifact_id == table.provenance_artifact_id()
            || table.expected_entities_artifact_id() != graph.expected_patches_artifact_id
            || table.expected_entities_logical_digest() != graph.expected_patches_logical_digest
            || table.support_artifact_id() != graph.patch_support_artifact_id
            || table.support_logical_digest() != graph.patch_support_logical_digest
            || table.provenance_artifact_id() != graph.provenance_artifact_id
            || table.provenance_logical_digest() != graph.provenance_logical_digest
            || table.dimension() != graph.output_dimension
            || support.artifact_id != graph.patch_support_artifact_id
            || support.logical_digest != graph.patch_support_logical_digest
            || support.expected_patches_artifact_id != graph.expected_patches_artifact_id
            || support.patch_context_artifact_id != graph.patch_context_artifact_id
            || support.patch_footprints_artifact_id != graph.patch_footprints_artifact_id
            || support.patch_overlap_artifact_id != graph.patch_overlap_artifact_id
            || support.row_count != row_count
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            artifact_id,
            logical_digest: table.logical_digest(),
            qc_summary: table.qc_summary(),
            expected_patches_artifact_id: graph.expected_patches_artifact_id,
            support_artifact_id: graph.patch_support_artifact_id,
            provenance_artifact_id: graph.provenance_artifact_id,
            source_row_link_artifact_id: graph.source_row_link_artifact_id,
            dimension: table.dimension(),
        })
    }

    /// Exact physical matrix artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent patch-table logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    /// Recomputed factual row/status/dimension/logical summary.
    pub fn qc_summary(self) -> MultiscaleEmbeddingQcSummary {
        self.qc_summary
    }

    /// Exact validated patch row count.
    pub fn row_count(self) -> u64 {
        self.qc_summary.row_count()
    }

    /// Exact validated output dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }
}

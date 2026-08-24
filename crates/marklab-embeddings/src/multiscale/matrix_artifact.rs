use std::fmt;

use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

use super::{digest::LogicalDigest, MultiscaleEmbeddingQcSummary};
#[cfg(feature = "parquet")]
use super::{
    DerivedRegionEmbeddingTableCandidate, DerivedSlideEmbeddingTableCandidate, EmbeddingEntityKind,
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
    expected_patches_logical_digest: ContentDigest,
    patch_context_artifact_id: ArtifactId,
    patch_footprints_artifact_id: ArtifactId,
    patch_overlap_artifact_id: ArtifactId,
    row_count: u64,
}

#[derive(Clone, Copy)]
#[cfg(feature = "parquet")]
pub(in crate::multiscale) struct VerifiedPatchEmbeddingSupportBindings {
    pub(in crate::multiscale) artifact_id: ArtifactId,
    pub(in crate::multiscale) logical_digest: ContentDigest,
    pub(in crate::multiscale) expected_patches_artifact_id: ArtifactId,
    pub(in crate::multiscale) expected_patches_logical_digest: ContentDigest,
    pub(in crate::multiscale) patch_context_artifact_id: ArtifactId,
    pub(in crate::multiscale) patch_footprints_artifact_id: ArtifactId,
    pub(in crate::multiscale) patch_overlap_artifact_id: ArtifactId,
    pub(in crate::multiscale) row_count: u64,
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
            expected_patches_logical_digest: graph.expected_patches_logical_digest,
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

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn bindings(self) -> VerifiedPatchEmbeddingSupportBindings {
        VerifiedPatchEmbeddingSupportBindings {
            artifact_id: self.artifact_id,
            logical_digest: self.logical_digest,
            expected_patches_artifact_id: self.expected_patches_artifact_id,
            expected_patches_logical_digest: self.expected_patches_logical_digest,
            patch_context_artifact_id: self.patch_context_artifact_id,
            patch_footprints_artifact_id: self.patch_footprints_artifact_id,
            patch_overlap_artifact_id: self.patch_overlap_artifact_id,
            row_count: self.row_count,
        }
    }
}

/// Runtime-only proof of one exact managed `region_from_patches` support artifact.
///
/// The receipt carries producer-declared transitive support only. It does not establish region
/// geometry, a tissue mask, or a full observation window.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedRegionEmbeddingSupportArtifact {
    artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    patch_support_artifact_id: ArtifactId,
    patch_support_logical_digest: ContentDigest,
    expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    patch_context_artifact_id: ArtifactId,
    patch_footprints_artifact_id: ArtifactId,
    patch_overlap_artifact_id: ArtifactId,
    patch_region_link_artifact_id: ArtifactId,
    patch_region_link_logical_digest: ContentDigest,
    expected_regions_artifact_id: ArtifactId,
    expected_regions_logical_digest: ContentDigest,
    link_converter_artifact_id: ArtifactId,
    link_assessment_artifact_id: ArtifactId,
    assessed_pair_count: u64,
    nonzero_relation_count: u64,
}

impl fmt::Debug for VerifiedRegionEmbeddingSupportArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedRegionEmbeddingSupportArtifact")
            .field("assessed_pair_count", &self.assessed_pair_count)
            .field("nonzero_relation_count", &self.nonzero_relation_count)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
#[cfg(feature = "parquet")]
pub(in crate::multiscale) struct VerifiedRegionEmbeddingSupportBindings {
    pub(in crate::multiscale) artifact_id: ArtifactId,
    pub(in crate::multiscale) logical_digest: ContentDigest,
    pub(in crate::multiscale) patch_support_artifact_id: ArtifactId,
    pub(in crate::multiscale) patch_support_logical_digest: ContentDigest,
    pub(in crate::multiscale) expected_patches_artifact_id: ArtifactId,
    pub(in crate::multiscale) expected_patches_logical_digest: ContentDigest,
    pub(in crate::multiscale) patch_context_artifact_id: ArtifactId,
    pub(in crate::multiscale) patch_footprints_artifact_id: ArtifactId,
    pub(in crate::multiscale) patch_region_link_artifact_id: ArtifactId,
    pub(in crate::multiscale) patch_region_link_logical_digest: ContentDigest,
    pub(in crate::multiscale) expected_regions_artifact_id: ArtifactId,
    pub(in crate::multiscale) expected_regions_logical_digest: ContentDigest,
    pub(in crate::multiscale) link_converter_artifact_id: ArtifactId,
    pub(in crate::multiscale) link_assessment_artifact_id: ArtifactId,
    pub(in crate::multiscale) nonzero_relation_count: u64,
}

impl VerifiedRegionEmbeddingSupportArtifact {
    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn new(
        artifact_id: ArtifactId,
        logical_digest: ContentDigest,
        patch_support: VerifiedPatchEmbeddingSupportBindings,
        patch_region_link_artifact_id: ArtifactId,
        link: &super::PatchRegionLink,
        nonzero_relation_count: u64,
    ) -> Self {
        Self {
            artifact_id,
            logical_digest,
            patch_support_artifact_id: patch_support.artifact_id,
            patch_support_logical_digest: patch_support.logical_digest,
            expected_patches_artifact_id: patch_support.expected_patches_artifact_id,
            expected_patches_logical_digest: patch_support.expected_patches_logical_digest,
            patch_context_artifact_id: patch_support.patch_context_artifact_id,
            patch_footprints_artifact_id: patch_support.patch_footprints_artifact_id,
            patch_overlap_artifact_id: patch_support.patch_overlap_artifact_id,
            patch_region_link_artifact_id,
            patch_region_link_logical_digest: link.logical_digest(),
            expected_regions_artifact_id: link.expected_regions_artifact_id(),
            expected_regions_logical_digest: link.expected_regions_logical_digest(),
            link_converter_artifact_id: link.converter_artifact_id(),
            link_assessment_artifact_id: link.assessment_artifact_id(),
            assessed_pair_count: link.assessed_pair_count(),
            nonzero_relation_count,
        }
    }

    /// Exact region-support artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent region-support logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn bindings(self) -> VerifiedRegionEmbeddingSupportBindings {
        VerifiedRegionEmbeddingSupportBindings {
            artifact_id: self.artifact_id,
            logical_digest: self.logical_digest,
            patch_support_artifact_id: self.patch_support_artifact_id,
            patch_support_logical_digest: self.patch_support_logical_digest,
            expected_patches_artifact_id: self.expected_patches_artifact_id,
            expected_patches_logical_digest: self.expected_patches_logical_digest,
            patch_context_artifact_id: self.patch_context_artifact_id,
            patch_footprints_artifact_id: self.patch_footprints_artifact_id,
            patch_region_link_artifact_id: self.patch_region_link_artifact_id,
            patch_region_link_logical_digest: self.patch_region_link_logical_digest,
            expected_regions_artifact_id: self.expected_regions_artifact_id,
            expected_regions_logical_digest: self.expected_regions_logical_digest,
            link_converter_artifact_id: self.link_converter_artifact_id,
            link_assessment_artifact_id: self.link_assessment_artifact_id,
            nonzero_relation_count: self.nonzero_relation_count,
        }
    }
}

/// Runtime-only proof of one exact managed slide-support artifact and its selected verified
/// lower-level table/support chain.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedSlideEmbeddingSupportArtifact {
    artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    owning_slide_binding_digest: ContentDigest,
    source_entity_kind: super::EmbeddingEntityKind,
    source_support_artifact_id: ArtifactId,
    source_support_logical_digest: ContentDigest,
    source_table_artifact_id: ArtifactId,
    source_table_logical_digest: ContentDigest,
    source_expected_artifact_id: ArtifactId,
    source_provenance_artifact_id: ArtifactId,
    source_row_count: u64,
    source_dimension: u32,
}

impl fmt::Debug for VerifiedSlideEmbeddingSupportArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedSlideEmbeddingSupportArtifact")
            .field("source_entity_kind", &self.source_entity_kind)
            .field("source_row_count", &self.source_row_count)
            .field("source_dimension", &self.source_dimension)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
#[cfg(feature = "parquet")]
pub(in crate::multiscale) struct VerifiedSlideEmbeddingSupportBindings {
    pub(in crate::multiscale) artifact_id: ArtifactId,
    pub(in crate::multiscale) logical_digest: ContentDigest,
    pub(in crate::multiscale) owning_slide_binding_digest: ContentDigest,
    pub(in crate::multiscale) source_entity_kind: EmbeddingEntityKind,
    pub(in crate::multiscale) source_support_artifact_id: ArtifactId,
    pub(in crate::multiscale) source_support_logical_digest: ContentDigest,
    pub(in crate::multiscale) source_table_artifact_id: ArtifactId,
    pub(in crate::multiscale) source_table_logical_digest: ContentDigest,
    pub(in crate::multiscale) source_expected_artifact_id: ArtifactId,
    pub(in crate::multiscale) source_provenance_artifact_id: ArtifactId,
    pub(in crate::multiscale) source_row_count: u64,
    pub(in crate::multiscale) source_dimension: u32,
}

impl VerifiedSlideEmbeddingSupportArtifact {
    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn from_patches(
        artifact_id: ArtifactId,
        logical_digest: ContentDigest,
        owning_slide_id: &SlideId,
        support: VerifiedPatchEmbeddingSupportBindings,
        source: VerifiedPatchEmbeddingTableBindings,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        if source.support_artifact_id != support.artifact_id
            || source.support_logical_digest != support.logical_digest
            || source.expected_patches_artifact_id != support.expected_patches_artifact_id
            || source.expected_patches_logical_digest != support.expected_patches_logical_digest
            || source.qc_summary.row_count() != support.row_count
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self::new(
            artifact_id,
            logical_digest,
            slide_lineage_digest(owning_slide_id),
            EmbeddingEntityKind::Patch,
            support.artifact_id,
            support.logical_digest,
            source.artifact_id,
            source.logical_digest,
            source.expected_patches_artifact_id,
            source.provenance_artifact_id,
            source.qc_summary.row_count(),
            source.dimension,
        ))
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn from_regions(
        artifact_id: ArtifactId,
        logical_digest: ContentDigest,
        owning_slide_id: &SlideId,
        support: VerifiedRegionEmbeddingSupportBindings,
        source: VerifiedRegionEmbeddingTableBindings,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        if source.support_artifact_id != support.artifact_id
            || source.support_logical_digest != support.logical_digest
            || source.expected_regions_artifact_id != support.expected_regions_artifact_id
            || source.expected_regions_logical_digest != support.expected_regions_logical_digest
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self::new(
            artifact_id,
            logical_digest,
            slide_lineage_digest(owning_slide_id),
            EmbeddingEntityKind::Region,
            support.artifact_id,
            support.logical_digest,
            source.artifact_id,
            source.logical_digest,
            source.expected_regions_artifact_id,
            source.provenance_artifact_id,
            source.qc_summary.row_count(),
            source.dimension,
        ))
    }

    #[cfg(feature = "parquet")]
    #[allow(clippy::too_many_arguments)]
    fn new(
        artifact_id: ArtifactId,
        logical_digest: ContentDigest,
        owning_slide_binding_digest: ContentDigest,
        source_entity_kind: super::EmbeddingEntityKind,
        source_support_artifact_id: ArtifactId,
        source_support_logical_digest: ContentDigest,
        source_table_artifact_id: ArtifactId,
        source_table_logical_digest: ContentDigest,
        source_expected_artifact_id: ArtifactId,
        source_provenance_artifact_id: ArtifactId,
        source_row_count: u64,
        source_dimension: u32,
    ) -> Self {
        Self {
            artifact_id,
            logical_digest,
            owning_slide_binding_digest,
            source_entity_kind,
            source_support_artifact_id,
            source_support_logical_digest,
            source_table_artifact_id,
            source_table_logical_digest,
            source_expected_artifact_id,
            source_provenance_artifact_id,
            source_row_count,
            source_dimension,
        }
    }

    /// Exact slide-support artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent slide-support logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn bindings(self) -> VerifiedSlideEmbeddingSupportBindings {
        VerifiedSlideEmbeddingSupportBindings {
            artifact_id: self.artifact_id,
            logical_digest: self.logical_digest,
            owning_slide_binding_digest: self.owning_slide_binding_digest,
            source_entity_kind: self.source_entity_kind,
            source_support_artifact_id: self.source_support_artifact_id,
            source_support_logical_digest: self.source_support_logical_digest,
            source_table_artifact_id: self.source_table_artifact_id,
            source_table_logical_digest: self.source_table_logical_digest,
            source_expected_artifact_id: self.source_expected_artifact_id,
            source_provenance_artifact_id: self.source_provenance_artifact_id,
            source_row_count: self.source_row_count,
            source_dimension: self.source_dimension,
        }
    }
}

pub(in crate::multiscale) fn slide_lineage_digest(slide_id: &SlideId) -> ContentDigest {
    let mut digest = LogicalDigest::new(b"marklab-slide-lineage-binding-v1");
    digest.text(slide_id.as_str());
    digest.finish()
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
    expected_patches_logical_digest: ContentDigest,
    support_artifact_id: ArtifactId,
    support_logical_digest: ContentDigest,
    provenance_artifact_id: ArtifactId,
    provenance_logical_digest: ContentDigest,
    source_row_link_artifact_id: ArtifactId,
    dimension: u32,
}

#[derive(Clone, Copy)]
#[cfg(feature = "parquet")]
pub(in crate::multiscale) struct VerifiedPatchEmbeddingTableBindings {
    pub(in crate::multiscale) artifact_id: ArtifactId,
    pub(in crate::multiscale) logical_digest: ContentDigest,
    pub(in crate::multiscale) qc_summary: MultiscaleEmbeddingQcSummary,
    pub(in crate::multiscale) expected_patches_artifact_id: ArtifactId,
    pub(in crate::multiscale) expected_patches_logical_digest: ContentDigest,
    pub(in crate::multiscale) support_artifact_id: ArtifactId,
    pub(in crate::multiscale) support_logical_digest: ContentDigest,
    pub(in crate::multiscale) provenance_artifact_id: ArtifactId,
    pub(in crate::multiscale) provenance_logical_digest: ContentDigest,
    pub(in crate::multiscale) dimension: u32,
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
            expected_patches_logical_digest: graph.expected_patches_logical_digest,
            support_artifact_id: graph.patch_support_artifact_id,
            support_logical_digest: graph.patch_support_logical_digest,
            provenance_artifact_id: graph.provenance_artifact_id,
            provenance_logical_digest: graph.provenance_logical_digest,
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

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn bindings(self) -> VerifiedPatchEmbeddingTableBindings {
        VerifiedPatchEmbeddingTableBindings {
            artifact_id: self.artifact_id,
            logical_digest: self.logical_digest,
            qc_summary: self.qc_summary,
            expected_patches_artifact_id: self.expected_patches_artifact_id,
            expected_patches_logical_digest: self.expected_patches_logical_digest,
            support_artifact_id: self.support_artifact_id,
            support_logical_digest: self.support_logical_digest,
            provenance_artifact_id: self.provenance_artifact_id,
            provenance_logical_digest: self.provenance_logical_digest,
            dimension: self.dimension,
        }
    }
}

/// Runtime-only proof of one fully decoded deterministically derived region embedding table.
///
/// This receipt binds the exact physical output to its recomputed logical/QC identity and the
/// complete derived-region lineage retained by the finalization candidate.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedRegionEmbeddingTableArtifact {
    artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    qc_summary: MultiscaleEmbeddingQcSummary,
    expected_regions_artifact_id: ArtifactId,
    expected_regions_logical_digest: ContentDigest,
    region_support_artifact_id: ArtifactId,
    region_support_logical_digest: ContentDigest,
    provenance_artifact_id: ArtifactId,
    provenance_logical_digest: ContentDigest,
    source_patch_table_artifact_id: ArtifactId,
    source_patch_table_logical_digest: ContentDigest,
    patch_region_link_artifact_id: ArtifactId,
    patch_region_link_logical_digest: ContentDigest,
    derivation_artifact_id: ArtifactId,
    derivation_logical_digest: ContentDigest,
    dimension: u32,
}

#[derive(Clone, Copy)]
#[cfg(feature = "parquet")]
pub(in crate::multiscale) struct VerifiedRegionEmbeddingTableBindings {
    pub(in crate::multiscale) artifact_id: ArtifactId,
    pub(in crate::multiscale) logical_digest: ContentDigest,
    pub(in crate::multiscale) qc_summary: MultiscaleEmbeddingQcSummary,
    pub(in crate::multiscale) expected_regions_artifact_id: ArtifactId,
    pub(in crate::multiscale) expected_regions_logical_digest: ContentDigest,
    pub(in crate::multiscale) support_artifact_id: ArtifactId,
    pub(in crate::multiscale) support_logical_digest: ContentDigest,
    pub(in crate::multiscale) provenance_artifact_id: ArtifactId,
    pub(in crate::multiscale) provenance_logical_digest: ContentDigest,
    pub(in crate::multiscale) dimension: u32,
}

impl fmt::Debug for VerifiedRegionEmbeddingTableArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedRegionEmbeddingTableArtifact")
            .field("row_count", &self.qc_summary.row_count())
            .field("dimension", &self.dimension)
            .finish_non_exhaustive()
    }
}

impl VerifiedRegionEmbeddingTableArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        artifact_id: ArtifactId,
        candidate: &DerivedRegionEmbeddingTableCandidate,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        let table = candidate.table();
        let graph = candidate.graph;
        if [
            graph.expected_regions_artifact_id,
            graph.region_support_artifact_id,
            graph.provenance_artifact_id,
            graph.source_patch_table_artifact_id,
            graph.patch_region_link_artifact_id,
            graph.derivation_artifact_id,
        ]
        .contains(&artifact_id)
            || table.expected_entities_artifact_id() != graph.expected_regions_artifact_id
            || table.expected_entities_logical_digest() != graph.expected_regions_logical_digest
            || table.support_artifact_id() != graph.region_support_artifact_id
            || table.support_logical_digest() != graph.region_support_logical_digest
            || table.provenance_artifact_id() != graph.provenance_artifact_id
            || table.provenance_logical_digest() != graph.provenance_logical_digest
            || table.dimension() != graph.output_dimension
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            artifact_id,
            logical_digest: table.logical_digest(),
            qc_summary: table.qc_summary(),
            expected_regions_artifact_id: graph.expected_regions_artifact_id,
            expected_regions_logical_digest: graph.expected_regions_logical_digest,
            region_support_artifact_id: graph.region_support_artifact_id,
            region_support_logical_digest: graph.region_support_logical_digest,
            provenance_artifact_id: graph.provenance_artifact_id,
            provenance_logical_digest: graph.provenance_logical_digest,
            source_patch_table_artifact_id: graph.source_patch_table_artifact_id,
            source_patch_table_logical_digest: graph.source_patch_table_logical_digest,
            patch_region_link_artifact_id: graph.patch_region_link_artifact_id,
            patch_region_link_logical_digest: graph.patch_region_link_logical_digest,
            derivation_artifact_id: graph.derivation_artifact_id,
            derivation_logical_digest: graph.derivation_logical_digest,
            dimension: graph.output_dimension,
        })
    }

    /// Exact physical matrix artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent region-table logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    /// Recomputed factual row/status/dimension/logical summary.
    pub fn qc_summary(self) -> MultiscaleEmbeddingQcSummary {
        self.qc_summary
    }

    /// Exact validated region row count.
    pub fn row_count(self) -> u64 {
        self.qc_summary.row_count()
    }

    /// Exact validated output dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn bindings(self) -> VerifiedRegionEmbeddingTableBindings {
        VerifiedRegionEmbeddingTableBindings {
            artifact_id: self.artifact_id,
            logical_digest: self.logical_digest,
            qc_summary: self.qc_summary,
            expected_regions_artifact_id: self.expected_regions_artifact_id,
            expected_regions_logical_digest: self.expected_regions_logical_digest,
            support_artifact_id: self.region_support_artifact_id,
            support_logical_digest: self.region_support_logical_digest,
            provenance_artifact_id: self.provenance_artifact_id,
            provenance_logical_digest: self.provenance_logical_digest,
            dimension: self.dimension,
        }
    }

    /// Exact expected-region-set artifact identity retained by finalization.
    pub fn expected_regions_artifact_id(self) -> ArtifactId {
        self.expected_regions_artifact_id
    }

    /// Format-independent expected-region-set identity retained by finalization.
    pub fn expected_regions_logical_digest(self) -> ContentDigest {
        self.expected_regions_logical_digest
    }

    /// Exact region-support artifact identity retained by finalization.
    pub fn region_support_artifact_id(self) -> ArtifactId {
        self.region_support_artifact_id
    }

    /// Format-independent region-support identity retained by finalization.
    pub fn region_support_logical_digest(self) -> ContentDigest {
        self.region_support_logical_digest
    }

    /// Exact derived-region provenance artifact identity retained by finalization.
    pub fn provenance_artifact_id(self) -> ArtifactId {
        self.provenance_artifact_id
    }

    /// Format-independent derived-region provenance identity retained by finalization.
    pub fn provenance_logical_digest(self) -> ContentDigest {
        self.provenance_logical_digest
    }

    /// Exact source patch-table artifact identity retained by finalization.
    pub fn source_patch_table_artifact_id(self) -> ArtifactId {
        self.source_patch_table_artifact_id
    }

    /// Format-independent source patch-table identity retained by finalization.
    pub fn source_patch_table_logical_digest(self) -> ContentDigest {
        self.source_patch_table_logical_digest
    }

    /// Exact patch-region-link artifact identity retained by finalization.
    pub fn patch_region_link_artifact_id(self) -> ArtifactId {
        self.patch_region_link_artifact_id
    }

    /// Format-independent patch-region-link identity retained by finalization.
    pub fn patch_region_link_logical_digest(self) -> ContentDigest {
        self.patch_region_link_logical_digest
    }

    /// Exact weighted-mean derivation artifact identity retained by finalization.
    pub fn derivation_artifact_id(self) -> ArtifactId {
        self.derivation_artifact_id
    }

    /// Format-independent weighted-mean derivation identity retained by finalization.
    pub fn derivation_logical_digest(self) -> ContentDigest {
        self.derivation_logical_digest
    }
}

/// Runtime-only proof of one fully decoded deterministically derived slide embedding table.
///
/// This receipt binds the singleton physical output to its recomputed logical/QC identity and the
/// selected patch- or region-sourced lineage retained by finalization.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedSlideEmbeddingTableArtifact {
    artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    qc_summary: MultiscaleEmbeddingQcSummary,
    expected_slides_artifact_id: ArtifactId,
    expected_slides_logical_digest: ContentDigest,
    slide_support_artifact_id: ArtifactId,
    slide_support_logical_digest: ContentDigest,
    provenance_artifact_id: ArtifactId,
    provenance_logical_digest: ContentDigest,
    source_table_artifact_id: ArtifactId,
    source_table_logical_digest: ContentDigest,
    source_support_artifact_id: ArtifactId,
    source_support_logical_digest: ContentDigest,
    derivation_artifact_id: ArtifactId,
    derivation_logical_digest: ContentDigest,
    dimension: u32,
}

impl fmt::Debug for VerifiedSlideEmbeddingTableArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedSlideEmbeddingTableArtifact")
            .field("row_count", &self.qc_summary.row_count())
            .field("dimension", &self.dimension)
            .finish_non_exhaustive()
    }
}

impl VerifiedSlideEmbeddingTableArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        artifact_id: ArtifactId,
        candidate: &DerivedSlideEmbeddingTableCandidate,
    ) -> Result<Self, crate::columnar::MultiscaleColumnarError> {
        let table = candidate.table();
        let graph = candidate.graph;
        if [
            graph.expected_slides_artifact_id,
            graph.slide_support_artifact_id,
            graph.provenance_artifact_id,
            graph.source_table_artifact_id,
            graph.source_support_artifact_id,
            graph.derivation_artifact_id,
        ]
        .contains(&artifact_id)
            || table.expected_entities_artifact_id() != graph.expected_slides_artifact_id
            || table.expected_entities_logical_digest() != graph.expected_slides_logical_digest
            || table.support_artifact_id() != graph.slide_support_artifact_id
            || table.support_logical_digest() != graph.slide_support_logical_digest
            || table.provenance_artifact_id() != graph.provenance_artifact_id
            || table.provenance_logical_digest() != graph.provenance_logical_digest
            || table.dimension() != graph.output_dimension
        {
            return Err(crate::columnar::MultiscaleColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            artifact_id,
            logical_digest: table.logical_digest(),
            qc_summary: table.qc_summary(),
            expected_slides_artifact_id: graph.expected_slides_artifact_id,
            expected_slides_logical_digest: graph.expected_slides_logical_digest,
            slide_support_artifact_id: graph.slide_support_artifact_id,
            slide_support_logical_digest: graph.slide_support_logical_digest,
            provenance_artifact_id: graph.provenance_artifact_id,
            provenance_logical_digest: graph.provenance_logical_digest,
            source_table_artifact_id: graph.source_table_artifact_id,
            source_table_logical_digest: graph.source_table_logical_digest,
            source_support_artifact_id: graph.source_support_artifact_id,
            source_support_logical_digest: graph.source_support_logical_digest,
            derivation_artifact_id: graph.derivation_artifact_id,
            derivation_logical_digest: graph.derivation_logical_digest,
            dimension: graph.output_dimension,
        })
    }

    /// Exact physical matrix artifact identity.
    pub fn artifact_id(self) -> ArtifactId {
        self.artifact_id
    }

    /// Format-independent slide-table logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    /// Recomputed factual row/status/dimension/logical summary.
    pub fn qc_summary(self) -> MultiscaleEmbeddingQcSummary {
        self.qc_summary
    }

    /// Exact validated singleton slide row count.
    pub fn row_count(self) -> u64 {
        self.qc_summary.row_count()
    }

    /// Exact validated output dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Exact expected-slide-set artifact identity retained by finalization.
    pub fn expected_slides_artifact_id(self) -> ArtifactId {
        self.expected_slides_artifact_id
    }

    /// Format-independent expected-slide-set identity retained by finalization.
    pub fn expected_slides_logical_digest(self) -> ContentDigest {
        self.expected_slides_logical_digest
    }

    /// Exact slide-support artifact identity retained by finalization.
    pub fn slide_support_artifact_id(self) -> ArtifactId {
        self.slide_support_artifact_id
    }

    /// Format-independent slide-support identity retained by finalization.
    pub fn slide_support_logical_digest(self) -> ContentDigest {
        self.slide_support_logical_digest
    }

    /// Exact derived-slide provenance artifact identity retained by finalization.
    pub fn provenance_artifact_id(self) -> ArtifactId {
        self.provenance_artifact_id
    }

    /// Format-independent derived-slide provenance identity retained by finalization.
    pub fn provenance_logical_digest(self) -> ContentDigest {
        self.provenance_logical_digest
    }

    /// Exact selected source-table artifact identity retained by finalization.
    pub fn source_table_artifact_id(self) -> ArtifactId {
        self.source_table_artifact_id
    }

    /// Format-independent selected source-table identity retained by finalization.
    pub fn source_table_logical_digest(self) -> ContentDigest {
        self.source_table_logical_digest
    }

    /// Exact selected lower-support artifact identity retained by finalization.
    pub fn source_support_artifact_id(self) -> ArtifactId {
        self.source_support_artifact_id
    }

    /// Format-independent selected lower-support identity retained by finalization.
    pub fn source_support_logical_digest(self) -> ContentDigest {
        self.source_support_logical_digest
    }

    /// Exact arithmetic-mean derivation artifact identity retained by finalization.
    pub fn derivation_artifact_id(self) -> ArtifactId {
        self.derivation_artifact_id
    }

    /// Format-independent arithmetic-mean derivation identity retained by finalization.
    pub fn derivation_logical_digest(self) -> ContentDigest {
        self.derivation_logical_digest
    }
}

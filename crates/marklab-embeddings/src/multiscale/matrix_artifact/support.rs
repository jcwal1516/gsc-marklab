use super::*;

/// Runtime-only proof that one patch-support descriptor has fully decoded footprint and overlap
/// artifacts matching its exact structural graph.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedPatchEmbeddingSupportArtifact {
    pub(super) artifact_id: ArtifactId,
    pub(super) logical_digest: ContentDigest,
    pub(super) expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    pub(super) patch_context_artifact_id: ArtifactId,
    pub(super) patch_footprints_artifact_id: ArtifactId,
    pub(super) patch_overlap_artifact_id: ArtifactId,
    pub(super) row_count: u64,
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

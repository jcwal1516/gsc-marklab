use marklab_project::{ArtifactCatalog, ArtifactId, LocalArtifactStore};

use super::{
    managed::{require_available, require_canonical_payload},
    record::{
        require_dependencies, require_record_profile, require_source_matrix_profile,
        required_record,
    },
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
};
use crate::multiscale::{
    matrix_artifact::{
        VerifiedPatchEmbeddingSupportArtifact, VerifiedPatchEmbeddingTableArtifact,
        VerifiedRegionEmbeddingSupportArtifact, VerifiedRegionEmbeddingTableArtifact,
        VerifiedSlideEmbeddingSupportArtifact,
    },
    physical::MatrixPhysicalProfile,
    MultiscaleEmbeddingSupport, PatchEmbeddingTable, RegionEmbeddingTable,
};

impl MultiscaleEmbeddingSupport {
    /// Verify one managed `slide_from_patches` support record from the exact source table and
    /// patch-support receipts.
    ///
    /// # Errors
    ///
    /// Returns a redacted graph error for variant, binding, role, record, canonical-payload, or
    /// managed-availability drift.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_slide_from_patches_artifact(
        &self,
        support_artifact_id: ArtifactId,
        source_table: &PatchEmbeddingTable,
        patch_support: VerifiedPatchEmbeddingSupportArtifact,
        verified_source_table: VerifiedPatchEmbeddingTableArtifact,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
    ) -> Result<VerifiedSlideEmbeddingSupportArtifact, MultiscaleEmbeddingArtifactGraphError> {
        let declared = self.slide_from_patches_bindings().ok_or(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::SlideSupport,
            },
        )?;
        let lower = patch_support.bindings();
        let source = verified_source_table.bindings();
        if self.owning_slide_id() != source_table.owning_slide_id()
            || declared.patch_support.artifact_id() != lower.artifact_id
            || declared.patch_support.logical_digest() != lower.logical_digest
            || declared.source_patch_table.artifact_id() != source.artifact_id
            || declared.source_patch_table.logical_digest() != source.logical_digest
            || source_table.logical_digest() != source.logical_digest
            || source_table.qc_summary() != source.qc_summary
            || source_table.expected_entities_artifact_id() != source.expected_patches_artifact_id
            || source_table.expected_entities_logical_digest()
                != source.expected_patches_logical_digest
            || source_table.support_artifact_id() != source.support_artifact_id
            || source_table.support_logical_digest() != source.support_logical_digest
            || source_table.provenance_artifact_id() != source.provenance_artifact_id
            || source_table.provenance_logical_digest() != source.provenance_logical_digest
            || source_table.dimension() != source.dimension
            || source.support_artifact_id != lower.artifact_id
            || source.support_logical_digest != lower.logical_digest
            || source.expected_patches_artifact_id != lower.expected_patches_artifact_id
            || source.expected_patches_logical_digest != lower.expected_patches_logical_digest
            || source.qc_summary.row_count() != lower.row_count
        {
            return binding_mismatch(MultiscaleEmbeddingArtifactRole::SlideSupport);
        }
        validate_common(
            self,
            support_artifact_id,
            lower.artifact_id,
            source.artifact_id,
            source.expected_patches_artifact_id,
            source.support_artifact_id,
            source.provenance_artifact_id,
            source.qc_summary.row_count(),
            source.dimension,
            MultiscaleEmbeddingArtifactRole::PatchSupport,
            MatrixPhysicalProfile::Patch,
            [
                lower.patch_context_artifact_id,
                lower.patch_footprints_artifact_id,
                lower.patch_overlap_artifact_id,
            ],
            3,
            catalog,
            store,
        )?;
        VerifiedSlideEmbeddingSupportArtifact::from_patches(
            support_artifact_id,
            self.logical_digest(),
            source_table.owning_slide_id(),
            lower,
            source,
        )
        .map_err(|_| domain_mismatch(MultiscaleEmbeddingArtifactRole::SlideSupport))
    }

    /// Verify one managed `slide_from_regions` support record from the exact source table and
    /// region-support receipts.
    ///
    /// # Errors
    ///
    /// Returns a redacted graph error for variant, binding, role, record, canonical-payload, or
    /// managed-availability drift.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_slide_from_regions_artifact(
        &self,
        support_artifact_id: ArtifactId,
        source_table: &RegionEmbeddingTable,
        region_support: VerifiedRegionEmbeddingSupportArtifact,
        verified_source_table: VerifiedRegionEmbeddingTableArtifact,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
    ) -> Result<VerifiedSlideEmbeddingSupportArtifact, MultiscaleEmbeddingArtifactGraphError> {
        let declared = self.slide_from_regions_bindings().ok_or(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::SlideSupport,
            },
        )?;
        let lower = region_support.bindings();
        let source = verified_source_table.bindings();
        if self.owning_slide_id() != source_table.owning_slide_id()
            || declared.region_support.artifact_id() != lower.artifact_id
            || declared.region_support.logical_digest() != lower.logical_digest
            || declared.source_region_table.artifact_id() != source.artifact_id
            || declared.source_region_table.logical_digest() != source.logical_digest
            || source_table.logical_digest() != source.logical_digest
            || source_table.qc_summary() != source.qc_summary
            || source_table.expected_entities_artifact_id() != source.expected_regions_artifact_id
            || source_table.expected_entities_logical_digest()
                != source.expected_regions_logical_digest
            || source_table.support_artifact_id() != source.support_artifact_id
            || source_table.support_logical_digest() != source.support_logical_digest
            || source_table.provenance_artifact_id() != source.provenance_artifact_id
            || source_table.provenance_logical_digest() != source.provenance_logical_digest
            || source_table.dimension() != source.dimension
            || source.support_artifact_id != lower.artifact_id
            || source.support_logical_digest != lower.logical_digest
            || source.expected_regions_artifact_id != lower.expected_regions_artifact_id
            || source.expected_regions_logical_digest != lower.expected_regions_logical_digest
        {
            return binding_mismatch(MultiscaleEmbeddingArtifactRole::SlideSupport);
        }
        validate_common(
            self,
            support_artifact_id,
            lower.artifact_id,
            source.artifact_id,
            source.expected_regions_artifact_id,
            source.support_artifact_id,
            source.provenance_artifact_id,
            source.qc_summary.row_count(),
            source.dimension,
            MultiscaleEmbeddingArtifactRole::RegionSupport,
            MatrixPhysicalProfile::Region,
            [
                lower.patch_support_artifact_id,
                lower.patch_region_link_artifact_id,
                lower.patch_support_artifact_id,
            ],
            2,
            catalog,
            store,
        )?;
        VerifiedSlideEmbeddingSupportArtifact::from_regions(
            support_artifact_id,
            self.logical_digest(),
            source_table.owning_slide_id(),
            lower,
            source,
        )
        .map_err(|_| domain_mismatch(MultiscaleEmbeddingArtifactRole::SlideSupport))
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_common(
    support: &MultiscaleEmbeddingSupport,
    support_artifact_id: ArtifactId,
    lower_support_artifact_id: ArtifactId,
    source_table_artifact_id: ArtifactId,
    source_expected_artifact_id: ArtifactId,
    source_table_support_artifact_id: ArtifactId,
    source_provenance_artifact_id: ArtifactId,
    source_row_count: u64,
    source_dimension: u32,
    lower_support_role: MultiscaleEmbeddingArtifactRole,
    source_profile: MatrixPhysicalProfile,
    mut lower_support_dependencies: [ArtifactId; 3],
    lower_support_dependency_count: usize,
    catalog: &ArtifactCatalog,
    store: &LocalArtifactStore,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    if lower_support_artifact_id != source_table_support_artifact_id
        || [lower_support_artifact_id, source_table_artifact_id].contains(&support_artifact_id)
        || lower_support_artifact_id == source_table_artifact_id
    {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::SlideSupport);
    }
    let source_role = match source_profile {
        MatrixPhysicalProfile::Patch => MultiscaleEmbeddingArtifactRole::SourcePatchTable,
        MatrixPhysicalProfile::Region => MultiscaleEmbeddingArtifactRole::SourceRegionTable,
        MatrixPhysicalProfile::Slide => {
            return binding_mismatch(MultiscaleEmbeddingArtifactRole::SlideSupport);
        }
    };
    let support_record = required_record(
        catalog,
        MultiscaleEmbeddingArtifactRole::SlideSupport,
        support_artifact_id,
    )?;
    let lower_support_record =
        required_record(catalog, lower_support_role, lower_support_artifact_id)?;
    let source_record = required_record(catalog, source_role, source_table_artifact_id)?;
    require_record_profile(
        support_record,
        MultiscaleEmbeddingArtifactRole::SlideSupport,
        None,
    )?;
    require_record_profile(lower_support_record, lower_support_role, None)?;
    require_source_matrix_profile(
        source_record,
        source_role,
        source_profile,
        source_row_count,
        source_dimension,
    )?;

    let mut support_dependencies = [lower_support_artifact_id, source_table_artifact_id];
    support_dependencies.sort_unstable();
    require_dependencies(
        support_record,
        MultiscaleEmbeddingArtifactRole::SlideSupport,
        &support_dependencies,
    )?;
    lower_support_dependencies[..lower_support_dependency_count].sort_unstable();
    require_dependencies(
        lower_support_record,
        lower_support_role,
        &lower_support_dependencies[..lower_support_dependency_count],
    )?;
    let mut source_dependencies = [
        source_expected_artifact_id,
        source_table_support_artifact_id,
        source_provenance_artifact_id,
    ];
    source_dependencies.sort_unstable();
    require_dependencies(source_record, source_role, &source_dependencies)?;

    require_canonical_payload(
        store,
        support_record,
        MultiscaleEmbeddingArtifactRole::SlideSupport,
        |reader| support.compare_canonical_json_reader(reader),
    )?;
    require_available(store, lower_support_record, lower_support_role)?;
    require_available(store, source_record, source_role)?;
    Ok(())
}

fn binding_mismatch<T>(
    role: MultiscaleEmbeddingArtifactRole,
) -> Result<T, MultiscaleEmbeddingArtifactGraphError> {
    Err(domain_mismatch(role))
}

fn domain_mismatch(role: MultiscaleEmbeddingArtifactRole) -> MultiscaleEmbeddingArtifactGraphError {
    MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch { role }
}

use marklab_project::{ArtifactCatalog, ArtifactId, LocalArtifactStore};

use super::{
    derived_region::require_patch_region_profile,
    managed::{require_available, require_canonical_payload},
    record::{require_dependencies, require_record_profile, required_record},
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
};
use crate::{
    multiscale::{
        matrix_artifact::{
            VerifiedPatchEmbeddingSupportArtifact, VerifiedRegionEmbeddingSupportArtifact,
        },
        MultiscaleEmbeddingSupport, PatchRegionLink,
    },
    VerifiedPatchRegionLinkArtifact,
};

impl MultiscaleEmbeddingSupport {
    /// Verify one managed `region_from_patches` support record from exact lower-level receipts.
    ///
    /// This receipt preserves producer-declared support lineage. It does not establish region
    /// geometry, a tissue mask, or a complete observation window.
    ///
    /// # Errors
    ///
    /// Returns a redacted graph error for variant, binding, role, record, canonical-payload, or
    /// managed-availability drift.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_region_from_patches_artifact(
        &self,
        support_artifact_id: ArtifactId,
        patch_support: VerifiedPatchEmbeddingSupportArtifact,
        link: &PatchRegionLink,
        verified_link: VerifiedPatchRegionLinkArtifact,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
    ) -> Result<VerifiedRegionEmbeddingSupportArtifact, MultiscaleEmbeddingArtifactGraphError> {
        let support_bindings = self.region_from_patches_bindings().ok_or(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            },
        )?;
        let patch_bindings = patch_support.bindings();
        let nonzero_relation_count =
            u64::try_from(link.nonzero_relation_count()).map_err(|_| {
                MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                    role: MultiscaleEmbeddingArtifactRole::PatchRegionLink,
                }
            })?;
        if self.owning_slide_id() != link.owning_slide_id()
            || support_bindings.patch_support.artifact_id() != patch_bindings.artifact_id
            || support_bindings.patch_support.logical_digest() != patch_bindings.logical_digest
            || support_bindings.patch_region_link.artifact_id() != verified_link.artifact_id()
            || support_bindings.patch_region_link.logical_digest() != verified_link.logical_digest()
            || verified_link.artifact_id() == patch_bindings.artifact_id
            || verified_link.artifact_id() != support_bindings.patch_region_link.artifact_id()
            || verified_link.logical_digest() != link.logical_digest()
            || verified_link.assessed_pair_count() != link.assessed_pair_count()
            || verified_link.row_count() != nonzero_relation_count
            || patch_bindings.expected_patches_artifact_id != link.expected_patches_artifact_id()
            || patch_bindings.expected_patches_logical_digest
                != link.expected_patches_logical_digest()
            || patch_bindings.patch_context_artifact_id != link.patch_context_artifact_id()
            || patch_bindings.patch_footprints_artifact_id != link.patch_footprints_artifact_id()
        {
            return binding_mismatch(MultiscaleEmbeddingArtifactRole::RegionSupport);
        }
        if [patch_bindings.artifact_id, verified_link.artifact_id()].contains(&support_artifact_id)
        {
            return Err(MultiscaleEmbeddingArtifactGraphError::RoleAlias);
        }

        let region_support_record = required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::RegionSupport,
            support_artifact_id,
        )?;
        let patch_support_record = required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::PatchSupport,
            patch_bindings.artifact_id,
        )?;
        let link_record = required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            verified_link.artifact_id(),
        )?;
        require_record_profile(
            region_support_record,
            MultiscaleEmbeddingArtifactRole::RegionSupport,
            None,
        )?;
        require_record_profile(
            patch_support_record,
            MultiscaleEmbeddingArtifactRole::PatchSupport,
            None,
        )?;
        require_patch_region_profile(link_record, verified_link.row_count())?;

        let mut region_support_dependencies =
            [patch_bindings.artifact_id, verified_link.artifact_id()];
        region_support_dependencies.sort_unstable();
        require_dependencies(
            region_support_record,
            MultiscaleEmbeddingArtifactRole::RegionSupport,
            &region_support_dependencies,
        )?;
        let mut patch_support_dependencies = [
            patch_bindings.patch_context_artifact_id,
            patch_bindings.patch_footprints_artifact_id,
            patch_bindings.patch_overlap_artifact_id,
        ];
        patch_support_dependencies.sort_unstable();
        require_dependencies(
            patch_support_record,
            MultiscaleEmbeddingArtifactRole::PatchSupport,
            &patch_support_dependencies,
        )?;
        let mut link_dependencies = [
            link.expected_patches_artifact_id(),
            link.expected_regions_artifact_id(),
            link.patch_context_artifact_id(),
            link.patch_footprints_artifact_id(),
            link.converter_artifact_id(),
            link.assessment_artifact_id(),
        ];
        link_dependencies.sort_unstable();
        require_dependencies(
            link_record,
            MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            &link_dependencies,
        )?;

        require_canonical_payload(
            store,
            region_support_record,
            MultiscaleEmbeddingArtifactRole::RegionSupport,
            |reader| self.compare_canonical_json_reader(reader),
        )?;
        require_available(
            store,
            patch_support_record,
            MultiscaleEmbeddingArtifactRole::PatchSupport,
        )?;
        require_available(
            store,
            link_record,
            MultiscaleEmbeddingArtifactRole::PatchRegionLink,
        )?;

        Ok(VerifiedRegionEmbeddingSupportArtifact::new(
            support_artifact_id,
            self.logical_digest(),
            patch_bindings,
            verified_link.artifact_id(),
            link,
            nonzero_relation_count,
        ))
    }
}

fn binding_mismatch<T>(
    role: MultiscaleEmbeddingArtifactRole,
) -> Result<T, MultiscaleEmbeddingArtifactGraphError> {
    Err(MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch { role })
}

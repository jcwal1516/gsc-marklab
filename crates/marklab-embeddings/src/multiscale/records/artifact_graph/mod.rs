use marklab_project::{ArtifactCatalog, ArtifactId, LocalArtifactStore};

use super::{
    MultiscaleEmbeddingProvenance, MultiscaleEmbeddingSupport, PatchEmbeddingInputNormalization,
    PatchEmbeddingSourceRowLink, PatchIdentityMap, PatchSourceEntitySet,
};
use crate::multiscale::{
    context::PatchEmbeddingContext, expected::ExpectedPatchSet, footprint::PatchFootprintSet,
    overlap::PatchOverlapGraph,
};

mod bindings;
#[cfg(feature = "parquet")]
mod cell_patch;
mod error;
mod managed;
#[cfg(feature = "parquet")]
mod patch_region;
mod record;

use bindings::{role_ids, validate_domain_bindings, validate_record_dependencies};
#[cfg(feature = "parquet")]
pub use cell_patch::{
    CellPatchInputArtifactGraphError, CellPatchInputArtifactRole,
    VerifiedCellPatchInputArtifactGraph,
};
pub use error::{
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
    VerifiedDirectPatchEmbeddingArtifactGraph,
};
use managed::{require_available_for, require_canonical_payload, require_canonical_payload_for};
#[cfg(feature = "parquet")]
pub use patch_region::{
    PatchRegionInputArtifactGraphError, PatchRegionInputArtifactRole,
    VerifiedPatchRegionInputArtifactGraph,
};
use record::{require_record_profile, required_record};

impl MultiscaleEmbeddingProvenance {
    /// Validate the exact direct-patch structural graph and each required artifact's managed
    /// replica in the supplied store.
    ///
    /// This does not decode physical footprint, overlap, source-vector, or embedding-table bytes.
    ///
    /// # Errors
    ///
    /// Returns a role-only error when decoded values disagree, a record/profile/dependency or
    /// canonical payload differs, the checkpoint digest drifts, or the supplied store cannot
    /// verify one required managed replica.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_direct_patch_artifact_graph(
        &self,
        provenance_artifact_id: ArtifactId,
        expected_patches: &ExpectedPatchSet,
        source_entities: &PatchSourceEntitySet,
        identity_map: &PatchIdentityMap,
        source_row_link: &PatchEmbeddingSourceRowLink,
        input_normalization: &PatchEmbeddingInputNormalization,
        context: &PatchEmbeddingContext,
        footprints: &PatchFootprintSet,
        overlap: &PatchOverlapGraph,
        support: &MultiscaleEmbeddingSupport,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
    ) -> Result<VerifiedDirectPatchEmbeddingArtifactGraph, MultiscaleEmbeddingArtifactGraphError>
    {
        let roles = self
            .direct_patch_artifact_roles()
            .ok_or(MultiscaleEmbeddingArtifactGraphError::UnsupportedProvenanceVariant)?;
        let support_bindings = support.patch_bindings().ok_or(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchSupport,
            },
        )?;
        validate_domain_bindings(
            self,
            &roles,
            &support_bindings,
            expected_patches,
            source_entities,
            identity_map,
            source_row_link,
            context,
            footprints,
            overlap,
            support,
        )?;

        let footprint_rows = u64::try_from(footprints.row_count()).map_err(|_| {
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchFootprints,
            }
        })?;
        let overlap_rows = u64::try_from(overlap.edge_count()).map_err(|_| {
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchOverlapGraph,
            }
        })?;
        for (role, id) in role_ids(provenance_artifact_id, &roles, &support_bindings) {
            let rows = match role {
                MultiscaleEmbeddingArtifactRole::PatchFootprints => Some(footprint_rows),
                MultiscaleEmbeddingArtifactRole::PatchOverlapGraph => Some(overlap_rows),
                _ => None,
            };
            require_record_profile(required_record(catalog, role, id)?, role, rows)?;
        }

        let provenance_record = required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::Provenance,
            provenance_artifact_id,
        )?;
        let checkpoint_record = required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::Checkpoint,
            roles.checkpoint,
        )?;
        if checkpoint_record.content().digest() != roles.checkpoint_content_digest {
            return Err(MultiscaleEmbeddingArtifactGraphError::CheckpointDigestMismatch);
        }
        validate_record_dependencies(
            &roles,
            &support_bindings,
            identity_map,
            source_row_link,
            catalog,
            provenance_record,
        )?;

        require_canonical_payload(
            store,
            provenance_record,
            MultiscaleEmbeddingArtifactRole::Provenance,
            |reader| self.compare_canonical_json_reader(reader),
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::Checkpoint,
            roles.checkpoint,
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::SourceSnapshot,
            roles.source_snapshot,
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::LicenseRecord,
            roles.license_record,
        )?;
        require_canonical_payload_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::InputNormalization,
            roles.input_normalization,
            |reader| input_normalization.compare_canonical_json_reader(reader),
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::Preprocessing,
            roles.preprocessing,
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::RunConfig,
            roles.run_config,
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::Environment,
            roles.environment,
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::Converter,
            roles.converter,
        )?;
        require_canonical_payload_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::SourceEntities,
            roles.source_entities,
            |reader| source_entities.compare_canonical_json_reader(reader),
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::SourceVectors,
            roles.source_vectors,
        )?;
        require_canonical_payload_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::ExpectedPatches,
            roles.expected_patches,
            |reader| expected_patches.compare_canonical_json_reader(reader),
        )?;
        require_canonical_payload_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::IdentityMap,
            roles.identity_map,
            |reader| identity_map.compare_canonical_json_reader(reader),
        )?;
        require_canonical_payload_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::SourceRowLink,
            roles.source_row_link,
            |reader| source_row_link.compare_canonical_json_reader(reader),
        )?;
        require_canonical_payload_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::PatchContext,
            support_bindings.context.artifact_id(),
            |reader| context.compare_canonical_json_reader(reader),
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::PatchFootprints,
            support_bindings.footprints.artifact_id(),
        )?;
        require_available_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::PatchOverlapGraph,
            support_bindings.overlap.artifact_id(),
        )?;
        require_canonical_payload_for(
            store,
            catalog,
            MultiscaleEmbeddingArtifactRole::PatchSupport,
            roles.patch_support,
            |reader| support.compare_canonical_json_reader(reader),
        )?;

        Ok(VerifiedDirectPatchEmbeddingArtifactGraph {
            provenance_artifact_id,
            provenance_dependency_count: 14,
            source_entities_artifact_id: roles.source_entities,
            source_vectors_artifact_id: roles.source_vectors,
            expected_patches_artifact_id: roles.expected_patches,
            expected_patches_logical_digest: expected_patches.logical_digest(),
            patch_context_artifact_id: support_bindings.context.artifact_id(),
            patch_context_logical_digest: context.logical_digest(),
            patch_footprints_artifact_id: support_bindings.footprints.artifact_id(),
            patch_footprints_logical_digest: footprints.logical_digest(),
            patch_overlap_artifact_id: support_bindings.overlap.artifact_id(),
            patch_overlap_logical_digest: overlap.logical_digest(),
            identity_map_artifact_id: roles.identity_map,
            source_row_link_artifact_id: roles.source_row_link,
            source_row_link_logical_digest: source_row_link.logical_digest(),
            patch_support_artifact_id: roles.patch_support,
            patch_support_logical_digest: support.logical_digest(),
            converter_artifact_id: roles.converter,
            output_dimension: self.output_dimension(),
        })
    }
}

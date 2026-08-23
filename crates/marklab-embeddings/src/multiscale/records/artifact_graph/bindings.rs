use marklab_project::{ArtifactCatalog, ArtifactId, ArtifactRecord};

use super::{
    record::{require_dependencies, required_record},
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
};
use crate::multiscale::{
    context::PatchEmbeddingContext,
    expected::ExpectedPatchSet,
    footprint::PatchFootprintSet,
    overlap::PatchOverlapGraph,
    records::{
        provenance::DirectPatchArtifactRoles, support::PatchSupportBindings,
        MultiscaleEmbeddingProvenance, MultiscaleEmbeddingSupport, PatchEmbeddingSourceRowLink,
        PatchIdentityMap, PatchSourceEntitySet,
    },
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_domain_bindings(
    provenance: &MultiscaleEmbeddingProvenance,
    roles: &DirectPatchArtifactRoles,
    support_bindings: &PatchSupportBindings,
    expected: &ExpectedPatchSet,
    source_entities: &PatchSourceEntitySet,
    identity_map: &PatchIdentityMap,
    source_row_link: &PatchEmbeddingSourceRowLink,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    support: &MultiscaleEmbeddingSupport,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    if expected.owning_slide_id() != provenance.owning_slide_id() {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::ExpectedPatches);
    }
    if context.owning_slide_id() != provenance.owning_slide_id() {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::PatchContext);
    }
    if support.owning_slide_id() != provenance.owning_slide_id() {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::PatchSupport);
    }
    if identity_map.source_entities_artifact_id() != roles.source_entities
        || identity_map.source_entities_logical_digest() != source_entities.logical_digest()
        || identity_map.expected_patches_artifact_id() != roles.expected_patches
        || identity_map.expected_patches_logical_digest() != expected.logical_digest()
    {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::IdentityMap);
    }
    if source_row_link.source_entities_artifact_id() != roles.source_entities
        || source_row_link.source_entities_logical_digest() != source_entities.logical_digest()
        || source_row_link.source_vectors_artifact_id() != roles.source_vectors
        || source_row_link.expected_patches_artifact_id() != roles.expected_patches
        || source_row_link.expected_patches_logical_digest() != expected.logical_digest()
        || source_row_link.identity_map_artifact_id() != roles.identity_map
        || source_row_link.identity_map_logical_digest() != identity_map.logical_digest()
        || source_row_link.converter_artifact_id() != roles.converter
        || source_row_link.entries().len() != expected.ids().len()
        || source_row_link
            .entries()
            .iter()
            .zip(expected.ids())
            .any(|(entry, expected_id)| entry.patch_id() != expected_id)
    {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::SourceRowLink);
    }
    if footprints.expected_patches_artifact_id() != roles.expected_patches
        || footprints.expected_patches_logical_digest() != expected.logical_digest()
        || footprints.patch_context_artifact_id() != support_bindings.context.artifact_id()
        || footprints.patch_context_logical_digest() != context.logical_digest()
    {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::PatchFootprints);
    }
    if overlap.expected_patches_artifact_id() != roles.expected_patches
        || overlap.expected_patches_logical_digest() != expected.logical_digest()
        || overlap.patch_footprints_artifact_id() != support_bindings.footprints.artifact_id()
        || overlap.patch_footprints_logical_digest() != footprints.logical_digest()
    {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::PatchOverlapGraph);
    }
    if support_bindings.context.logical_digest() != context.logical_digest()
        || support_bindings.footprints.logical_digest() != footprints.logical_digest()
        || support_bindings.overlap.logical_digest() != overlap.logical_digest()
        || roles.patch_support == support_bindings.context.artifact_id()
        || roles.patch_support == support_bindings.footprints.artifact_id()
        || roles.patch_support == support_bindings.overlap.artifact_id()
    {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::PatchSupport);
    }
    Ok(())
}

fn binding_mismatch<T>(
    role: MultiscaleEmbeddingArtifactRole,
) -> Result<T, MultiscaleEmbeddingArtifactGraphError> {
    Err(MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch { role })
}

pub(super) fn role_ids(
    provenance_artifact_id: ArtifactId,
    roles: &DirectPatchArtifactRoles,
    support: &PatchSupportBindings,
) -> [(MultiscaleEmbeddingArtifactRole, ArtifactId); 18] {
    [
        (
            MultiscaleEmbeddingArtifactRole::Provenance,
            provenance_artifact_id,
        ),
        (
            MultiscaleEmbeddingArtifactRole::Checkpoint,
            roles.checkpoint,
        ),
        (
            MultiscaleEmbeddingArtifactRole::SourceSnapshot,
            roles.source_snapshot,
        ),
        (
            MultiscaleEmbeddingArtifactRole::LicenseRecord,
            roles.license_record,
        ),
        (
            MultiscaleEmbeddingArtifactRole::InputNormalization,
            roles.input_normalization,
        ),
        (
            MultiscaleEmbeddingArtifactRole::Preprocessing,
            roles.preprocessing,
        ),
        (MultiscaleEmbeddingArtifactRole::RunConfig, roles.run_config),
        (
            MultiscaleEmbeddingArtifactRole::Environment,
            roles.environment,
        ),
        (MultiscaleEmbeddingArtifactRole::Converter, roles.converter),
        (
            MultiscaleEmbeddingArtifactRole::SourceEntities,
            roles.source_entities,
        ),
        (
            MultiscaleEmbeddingArtifactRole::SourceVectors,
            roles.source_vectors,
        ),
        (
            MultiscaleEmbeddingArtifactRole::ExpectedPatches,
            roles.expected_patches,
        ),
        (
            MultiscaleEmbeddingArtifactRole::IdentityMap,
            roles.identity_map,
        ),
        (
            MultiscaleEmbeddingArtifactRole::SourceRowLink,
            roles.source_row_link,
        ),
        (
            MultiscaleEmbeddingArtifactRole::PatchContext,
            support.context.artifact_id(),
        ),
        (
            MultiscaleEmbeddingArtifactRole::PatchFootprints,
            support.footprints.artifact_id(),
        ),
        (
            MultiscaleEmbeddingArtifactRole::PatchOverlapGraph,
            support.overlap.artifact_id(),
        ),
        (
            MultiscaleEmbeddingArtifactRole::PatchSupport,
            roles.patch_support,
        ),
    ]
}

pub(super) fn validate_record_dependencies(
    roles: &DirectPatchArtifactRoles,
    support_bindings: &PatchSupportBindings,
    identity_map: &PatchIdentityMap,
    source_row_link: &PatchEmbeddingSourceRowLink,
    catalog: &ArtifactCatalog,
    provenance_record: &ArtifactRecord,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    for (role, id) in [
        (
            MultiscaleEmbeddingArtifactRole::Checkpoint,
            roles.checkpoint,
        ),
        (
            MultiscaleEmbeddingArtifactRole::SourceSnapshot,
            roles.source_snapshot,
        ),
        (
            MultiscaleEmbeddingArtifactRole::LicenseRecord,
            roles.license_record,
        ),
        (
            MultiscaleEmbeddingArtifactRole::InputNormalization,
            roles.input_normalization,
        ),
        (
            MultiscaleEmbeddingArtifactRole::Preprocessing,
            roles.preprocessing,
        ),
        (MultiscaleEmbeddingArtifactRole::RunConfig, roles.run_config),
        (
            MultiscaleEmbeddingArtifactRole::Environment,
            roles.environment,
        ),
        (MultiscaleEmbeddingArtifactRole::Converter, roles.converter),
        (
            MultiscaleEmbeddingArtifactRole::SourceEntities,
            roles.source_entities,
        ),
        (
            MultiscaleEmbeddingArtifactRole::SourceVectors,
            roles.source_vectors,
        ),
        (
            MultiscaleEmbeddingArtifactRole::ExpectedPatches,
            roles.expected_patches,
        ),
        (
            MultiscaleEmbeddingArtifactRole::PatchContext,
            support_bindings.context.artifact_id(),
        ),
    ] {
        require_dependencies(required_record(catalog, role, id)?, role, &[])?;
    }
    require_dependencies(
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::IdentityMap,
            roles.identity_map,
        )?,
        MultiscaleEmbeddingArtifactRole::IdentityMap,
        &identity_map.direct_dependencies(),
    )?;
    require_dependencies(
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::SourceRowLink,
            roles.source_row_link,
        )?,
        MultiscaleEmbeddingArtifactRole::SourceRowLink,
        &source_row_link.direct_dependencies(),
    )?;
    let mut footprint_dependencies = [
        roles.expected_patches,
        support_bindings.context.artifact_id(),
    ];
    footprint_dependencies.sort_unstable();
    require_dependencies(
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::PatchFootprints,
            support_bindings.footprints.artifact_id(),
        )?,
        MultiscaleEmbeddingArtifactRole::PatchFootprints,
        &footprint_dependencies,
    )?;
    let mut overlap_dependencies = [
        roles.expected_patches,
        support_bindings.context.artifact_id(),
        support_bindings.footprints.artifact_id(),
    ];
    overlap_dependencies.sort_unstable();
    require_dependencies(
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::PatchOverlapGraph,
            support_bindings.overlap.artifact_id(),
        )?,
        MultiscaleEmbeddingArtifactRole::PatchOverlapGraph,
        &overlap_dependencies,
    )?;
    let mut support_dependencies = [
        support_bindings.context.artifact_id(),
        support_bindings.footprints.artifact_id(),
        support_bindings.overlap.artifact_id(),
    ];
    support_dependencies.sort_unstable();
    require_dependencies(
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::PatchSupport,
            roles.patch_support,
        )?,
        MultiscaleEmbeddingArtifactRole::PatchSupport,
        &support_dependencies,
    )?;
    let mut provenance_dependencies = [
        roles.checkpoint,
        roles.source_snapshot,
        roles.license_record,
        roles.input_normalization,
        roles.preprocessing,
        roles.run_config,
        roles.environment,
        roles.converter,
        roles.source_entities,
        roles.source_vectors,
        roles.expected_patches,
        roles.identity_map,
        roles.source_row_link,
        roles.patch_support,
    ];
    provenance_dependencies.sort_unstable();
    require_dependencies(
        provenance_record,
        MultiscaleEmbeddingArtifactRole::Provenance,
        &provenance_dependencies,
    )
}

use marklab_project::{ArtifactCatalog, ArtifactId, LocalArtifactStore};

use super::{
    managed::{require_available, require_canonical_payload},
    record::{
        require_dependencies, require_record_profile, require_source_matrix_profile,
        required_record,
    },
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
    VerifiedDerivedSlideEmbeddingArtifactGraph,
};
use crate::multiscale::{
    matrix_artifact::slide_lineage_digest,
    physical::MatrixPhysicalProfile,
    records::{
        provenance::DerivedSlideArtifactRoles, MultiscaleEmbeddingDerivationContract,
        MultiscaleEmbeddingProvenance,
    },
    EmbeddingEntityKind, ExpectedSlideSet, VerifiedSlideEmbeddingSupportArtifact,
};

impl MultiscaleEmbeddingProvenance {
    /// Validate the exact patch-sourced derived-slide graph over one verified slide-support
    /// capability.
    ///
    /// # Errors
    ///
    /// Returns a role-only error for provenance variant, record, dependency, payload, managed
    /// availability, receipt, lineage, algorithm, or dimension drift.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_derived_slide_from_patches_artifact_graph(
        &self,
        provenance_artifact_id: ArtifactId,
        expected_slides: &ExpectedSlideSet,
        derivation: &MultiscaleEmbeddingDerivationContract,
        slide_support: VerifiedSlideEmbeddingSupportArtifact,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
    ) -> Result<VerifiedDerivedSlideEmbeddingArtifactGraph, MultiscaleEmbeddingArtifactGraphError>
    {
        let roles = self.derived_slide_from_patches_artifact_roles().ok_or(
            MultiscaleEmbeddingArtifactGraphError::UnsupportedDerivedSlideProvenanceVariant,
        )?;
        validate_derived_slide_graph(
            self,
            provenance_artifact_id,
            expected_slides,
            derivation,
            slide_support,
            roles,
            EmbeddingEntityKind::Patch,
            catalog,
            store,
        )
    }

    /// Validate the exact region-sourced derived-slide graph over one verified slide-support
    /// capability.
    ///
    /// # Errors
    ///
    /// Returns a role-only error for provenance variant, record, dependency, payload, managed
    /// availability, receipt, lineage, algorithm, or dimension drift.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_derived_slide_from_regions_artifact_graph(
        &self,
        provenance_artifact_id: ArtifactId,
        expected_slides: &ExpectedSlideSet,
        derivation: &MultiscaleEmbeddingDerivationContract,
        slide_support: VerifiedSlideEmbeddingSupportArtifact,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
    ) -> Result<VerifiedDerivedSlideEmbeddingArtifactGraph, MultiscaleEmbeddingArtifactGraphError>
    {
        let roles = self.derived_slide_from_regions_artifact_roles().ok_or(
            MultiscaleEmbeddingArtifactGraphError::UnsupportedDerivedSlideProvenanceVariant,
        )?;
        validate_derived_slide_graph(
            self,
            provenance_artifact_id,
            expected_slides,
            derivation,
            slide_support,
            roles,
            EmbeddingEntityKind::Region,
            catalog,
            store,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_derived_slide_graph(
    provenance: &MultiscaleEmbeddingProvenance,
    provenance_artifact_id: ArtifactId,
    expected_slides: &ExpectedSlideSet,
    derivation: &MultiscaleEmbeddingDerivationContract,
    slide_support: VerifiedSlideEmbeddingSupportArtifact,
    roles: DerivedSlideArtifactRoles,
    expected_source_kind: EmbeddingEntityKind,
    catalog: &ArtifactCatalog,
    store: &LocalArtifactStore,
) -> Result<VerifiedDerivedSlideEmbeddingArtifactGraph, MultiscaleEmbeddingArtifactGraphError> {
    let support = slide_support.bindings();
    let provenance_slide_binding = slide_lineage_digest(provenance.owning_slide_id());
    let expected_slide_binding = slide_lineage_digest(expected_slides.owning_slide_id());
    if provenance.owning_slide_id() != expected_slides.owning_slide_id()
        || support.owning_slide_binding_digest != provenance_slide_binding
        || support.owning_slide_binding_digest != expected_slide_binding
        || provenance.pooling_or_aggregation() != "arithmetic_mean"
        || derivation.algorithm() != "arithmetic_mean"
        || support.source_entity_kind != expected_source_kind
        || roles.source_table != support.source_table_artifact_id
        || roles.slide_support != support.artifact_id
        || support.source_dimension != provenance.output_dimension()
    {
        return binding_mismatch(MultiscaleEmbeddingArtifactRole::Provenance);
    }

    let source_role = source_role(expected_source_kind)?;
    let role_ids = [
        (
            MultiscaleEmbeddingArtifactRole::Provenance,
            provenance_artifact_id,
        ),
        (MultiscaleEmbeddingArtifactRole::RunConfig, roles.run_config),
        (
            MultiscaleEmbeddingArtifactRole::Environment,
            roles.environment,
        ),
        (MultiscaleEmbeddingArtifactRole::Converter, roles.converter),
        (source_role, roles.source_table),
        (
            MultiscaleEmbeddingArtifactRole::ExpectedSlides,
            roles.expected_slides,
        ),
        (
            MultiscaleEmbeddingArtifactRole::SlideSupport,
            roles.slide_support,
        ),
        (
            MultiscaleEmbeddingArtifactRole::Derivation,
            roles.derivation_contract,
        ),
    ];
    require_distinct_roles(&role_ids)?;
    for (role, id) in role_ids {
        let record = required_record(catalog, role, id)?;
        if role == source_role {
            require_source_matrix_profile(
                record,
                role,
                source_profile(expected_source_kind)?,
                support.source_row_count,
                support.source_dimension,
            )?;
        } else {
            require_record_profile(record, role, None)?;
        }
    }

    for (role, id) in [
        (MultiscaleEmbeddingArtifactRole::RunConfig, roles.run_config),
        (
            MultiscaleEmbeddingArtifactRole::Environment,
            roles.environment,
        ),
        (MultiscaleEmbeddingArtifactRole::Converter, roles.converter),
        (
            MultiscaleEmbeddingArtifactRole::ExpectedSlides,
            roles.expected_slides,
        ),
        (
            MultiscaleEmbeddingArtifactRole::Derivation,
            roles.derivation_contract,
        ),
    ] {
        require_dependencies(required_record(catalog, role, id)?, role, &[])?;
    }
    let mut source_dependencies = [
        support.source_expected_artifact_id,
        support.source_support_artifact_id,
        support.source_provenance_artifact_id,
    ];
    source_dependencies.sort_unstable();
    require_dependencies(
        required_record(catalog, source_role, roles.source_table)?,
        source_role,
        &source_dependencies,
    )?;
    let mut support_dependencies = [
        support.source_support_artifact_id,
        support.source_table_artifact_id,
    ];
    support_dependencies.sort_unstable();
    require_dependencies(
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::SlideSupport,
            roles.slide_support,
        )?,
        MultiscaleEmbeddingArtifactRole::SlideSupport,
        &support_dependencies,
    )?;
    let mut provenance_dependencies = [
        roles.run_config,
        roles.environment,
        roles.converter,
        roles.source_table,
        roles.expected_slides,
        roles.slide_support,
        roles.derivation_contract,
    ];
    provenance_dependencies.sort_unstable();
    require_dependencies(
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::Provenance,
            provenance_artifact_id,
        )?,
        MultiscaleEmbeddingArtifactRole::Provenance,
        &provenance_dependencies,
    )?;

    require_canonical_payload(
        store,
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::Provenance,
            provenance_artifact_id,
        )?,
        MultiscaleEmbeddingArtifactRole::Provenance,
        |reader| provenance.compare_canonical_json_reader(reader),
    )?;
    require_canonical_payload(
        store,
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::ExpectedSlides,
            roles.expected_slides,
        )?,
        MultiscaleEmbeddingArtifactRole::ExpectedSlides,
        |reader| expected_slides.compare_canonical_json_reader(reader),
    )?;
    require_canonical_payload(
        store,
        required_record(
            catalog,
            MultiscaleEmbeddingArtifactRole::Derivation,
            roles.derivation_contract,
        )?,
        MultiscaleEmbeddingArtifactRole::Derivation,
        |reader| derivation.compare_canonical_json_reader(reader),
    )?;
    for (role, id) in [
        (MultiscaleEmbeddingArtifactRole::RunConfig, roles.run_config),
        (
            MultiscaleEmbeddingArtifactRole::Environment,
            roles.environment,
        ),
        (MultiscaleEmbeddingArtifactRole::Converter, roles.converter),
        (source_role, roles.source_table),
        (
            MultiscaleEmbeddingArtifactRole::SlideSupport,
            roles.slide_support,
        ),
    ] {
        require_available(store, required_record(catalog, role, id)?, role)?;
    }

    Ok(VerifiedDerivedSlideEmbeddingArtifactGraph {
        provenance_artifact_id,
        provenance_logical_digest: provenance.logical_digest(),
        provenance_dependency_count: 7,
        owning_slide_binding_digest: support.owning_slide_binding_digest,
        source_entity_kind: expected_source_kind,
        source_table_artifact_id: support.source_table_artifact_id,
        source_table_logical_digest: support.source_table_logical_digest,
        source_table_row_count: support.source_row_count,
        source_support_artifact_id: support.source_support_artifact_id,
        source_support_logical_digest: support.source_support_logical_digest,
        expected_slides_artifact_id: roles.expected_slides,
        expected_slides_logical_digest: expected_slides.logical_digest(),
        slide_support_artifact_id: support.artifact_id,
        slide_support_logical_digest: support.logical_digest,
        derivation_artifact_id: roles.derivation_contract,
        derivation_logical_digest: derivation.logical_digest(),
        output_dimension: provenance.output_dimension(),
    })
}

fn source_role(
    kind: EmbeddingEntityKind,
) -> Result<MultiscaleEmbeddingArtifactRole, MultiscaleEmbeddingArtifactGraphError> {
    match kind {
        EmbeddingEntityKind::Patch => Ok(MultiscaleEmbeddingArtifactRole::SourcePatchTable),
        EmbeddingEntityKind::Region => Ok(MultiscaleEmbeddingArtifactRole::SourceRegionTable),
        EmbeddingEntityKind::Slide => binding_mismatch(MultiscaleEmbeddingArtifactRole::Provenance),
    }
}

fn source_profile(
    kind: EmbeddingEntityKind,
) -> Result<MatrixPhysicalProfile, MultiscaleEmbeddingArtifactGraphError> {
    match kind {
        EmbeddingEntityKind::Patch => Ok(MatrixPhysicalProfile::Patch),
        EmbeddingEntityKind::Region => Ok(MatrixPhysicalProfile::Region),
        EmbeddingEntityKind::Slide => binding_mismatch(MultiscaleEmbeddingArtifactRole::Provenance),
    }
}

fn require_distinct_roles(
    roles: &[(MultiscaleEmbeddingArtifactRole, ArtifactId); 8],
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    let mut ids = roles.map(|(_, id)| id);
    ids.sort_unstable();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(MultiscaleEmbeddingArtifactGraphError::RoleAlias);
    }
    Ok(())
}

fn binding_mismatch<T>(
    role: MultiscaleEmbeddingArtifactRole,
) -> Result<T, MultiscaleEmbeddingArtifactGraphError> {
    Err(MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch { role })
}

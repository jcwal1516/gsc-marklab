use marklab_project::{ArtifactCatalog, ArtifactId, ArtifactRecord, LocalArtifactStore};

use super::{
    managed::{require_available, require_canonical_payload},
    record::{require_dependencies, require_record_profile, required_record},
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
    VerifiedDerivedRegionEmbeddingArtifactGraph,
};
use crate::multiscale::{
    matrix_artifact::{
        VerifiedPatchEmbeddingTableArtifact, VerifiedRegionEmbeddingSupportArtifact,
    },
    physical::{
        content_kind as spatial_content_kind, record_matches, MatrixPhysicalProfile,
        SpatialArtifactRole, SpatialPhysicalEncoding,
    },
    records::{MultiscaleEmbeddingDerivationContract, MultiscaleEmbeddingProvenance},
    ExpectedRegionSet,
};

#[cfg(test)]
#[path = "derived_region_tests.rs"]
mod tests;

impl MultiscaleEmbeddingProvenance {
    /// Validate the exact derived-region graph over verified patch-table and region-support
    /// capabilities.
    ///
    /// This graph contains no output region table. A later finalizer must recompute every region
    /// row before a physical table receipt can be issued.
    ///
    /// # Errors
    ///
    /// Returns a role-only error for provenance variant, record, dependency, payload, managed
    /// availability, receipt, lineage, algorithm, or dimension drift.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_derived_region_artifact_graph(
        &self,
        provenance_artifact_id: ArtifactId,
        expected_regions: &ExpectedRegionSet,
        derivation: &MultiscaleEmbeddingDerivationContract,
        source_patch_table: VerifiedPatchEmbeddingTableArtifact,
        region_support: VerifiedRegionEmbeddingSupportArtifact,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
    ) -> Result<VerifiedDerivedRegionEmbeddingArtifactGraph, MultiscaleEmbeddingArtifactGraphError>
    {
        let roles = self.derived_region_artifact_roles().ok_or(
            MultiscaleEmbeddingArtifactGraphError::UnsupportedDerivedRegionProvenanceVariant,
        )?;
        let source = source_patch_table.bindings();
        let support = region_support.bindings();
        if self.owning_slide_id() != expected_regions.owning_slide_id()
            || self.pooling_or_aggregation() != "weighted_mean"
            || derivation.algorithm() != "weighted_mean"
            || roles.source_patch_table != source.artifact_id
            || roles.patch_region_link != support.patch_region_link_artifact_id
            || roles.expected_regions != support.expected_regions_artifact_id
            || roles.region_support != support.artifact_id
            || source.expected_patches_artifact_id != support.expected_patches_artifact_id
            || source.expected_patches_logical_digest != support.expected_patches_logical_digest
            || source.support_artifact_id != support.patch_support_artifact_id
            || source.support_logical_digest != support.patch_support_logical_digest
            || source.dimension != self.output_dimension()
            || support.expected_regions_logical_digest != expected_regions.logical_digest()
        {
            return binding_mismatch(MultiscaleEmbeddingArtifactRole::Provenance);
        }

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
            (
                MultiscaleEmbeddingArtifactRole::SourcePatchTable,
                roles.source_patch_table,
            ),
            (
                MultiscaleEmbeddingArtifactRole::PatchRegionLink,
                roles.patch_region_link,
            ),
            (
                MultiscaleEmbeddingArtifactRole::ExpectedRegions,
                roles.expected_regions,
            ),
            (
                MultiscaleEmbeddingArtifactRole::RegionSupport,
                roles.region_support,
            ),
            (
                MultiscaleEmbeddingArtifactRole::Derivation,
                roles.derivation_contract,
            ),
        ];
        require_distinct_roles(&role_ids)?;
        for (role, id) in role_ids {
            let record = required_record(catalog, role, id)?;
            match role {
                MultiscaleEmbeddingArtifactRole::SourcePatchTable => {
                    require_patch_table_profile(
                        record,
                        source.qc_summary.row_count(),
                        source.dimension,
                    )?;
                }
                MultiscaleEmbeddingArtifactRole::PatchRegionLink => {
                    require_patch_region_profile(record, support.nonzero_relation_count)?;
                }
                _ => require_record_profile(record, role, None)?,
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
                MultiscaleEmbeddingArtifactRole::ExpectedRegions,
                roles.expected_regions,
            ),
            (
                MultiscaleEmbeddingArtifactRole::Derivation,
                roles.derivation_contract,
            ),
        ] {
            require_dependencies(required_record(catalog, role, id)?, role, &[])?;
        }
        let mut source_dependencies = [
            source.expected_patches_artifact_id,
            source.support_artifact_id,
            source.provenance_artifact_id,
        ];
        source_dependencies.sort_unstable();
        require_dependencies(
            required_record(
                catalog,
                MultiscaleEmbeddingArtifactRole::SourcePatchTable,
                roles.source_patch_table,
            )?,
            MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            &source_dependencies,
        )?;
        let mut link_dependencies = [
            support.expected_patches_artifact_id,
            support.expected_regions_artifact_id,
            support.patch_context_artifact_id,
            support.patch_footprints_artifact_id,
            support.link_converter_artifact_id,
            support.link_assessment_artifact_id,
        ];
        link_dependencies.sort_unstable();
        require_dependencies(
            required_record(
                catalog,
                MultiscaleEmbeddingArtifactRole::PatchRegionLink,
                roles.patch_region_link,
            )?,
            MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            &link_dependencies,
        )?;
        let mut support_dependencies = [
            support.patch_support_artifact_id,
            support.patch_region_link_artifact_id,
        ];
        support_dependencies.sort_unstable();
        require_dependencies(
            required_record(
                catalog,
                MultiscaleEmbeddingArtifactRole::RegionSupport,
                roles.region_support,
            )?,
            MultiscaleEmbeddingArtifactRole::RegionSupport,
            &support_dependencies,
        )?;
        let mut provenance_dependencies = [
            roles.run_config,
            roles.environment,
            roles.converter,
            roles.source_patch_table,
            roles.patch_region_link,
            roles.expected_regions,
            roles.region_support,
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
            |reader| self.compare_canonical_json_reader(reader),
        )?;
        require_canonical_payload(
            store,
            required_record(
                catalog,
                MultiscaleEmbeddingArtifactRole::ExpectedRegions,
                roles.expected_regions,
            )?,
            MultiscaleEmbeddingArtifactRole::ExpectedRegions,
            |reader| expected_regions.compare_canonical_json_reader(reader),
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
            (
                MultiscaleEmbeddingArtifactRole::SourcePatchTable,
                roles.source_patch_table,
            ),
            (
                MultiscaleEmbeddingArtifactRole::PatchRegionLink,
                roles.patch_region_link,
            ),
            (
                MultiscaleEmbeddingArtifactRole::RegionSupport,
                roles.region_support,
            ),
        ] {
            require_available(store, required_record(catalog, role, id)?, role)?;
        }

        Ok(VerifiedDerivedRegionEmbeddingArtifactGraph {
            provenance_artifact_id,
            provenance_logical_digest: self.logical_digest(),
            provenance_dependency_count: 8,
            source_patch_table_artifact_id: source.artifact_id,
            source_patch_table_logical_digest: source.logical_digest,
            source_patch_provenance_logical_digest: source.provenance_logical_digest,
            source_patch_table_row_count: source.qc_summary.row_count(),
            source_expected_patches_artifact_id: source.expected_patches_artifact_id,
            source_patch_support_artifact_id: source.support_artifact_id,
            patch_region_link_artifact_id: support.patch_region_link_artifact_id,
            patch_region_link_logical_digest: support.patch_region_link_logical_digest,
            expected_regions_artifact_id: support.expected_regions_artifact_id,
            expected_regions_logical_digest: support.expected_regions_logical_digest,
            region_support_artifact_id: support.artifact_id,
            region_support_logical_digest: support.logical_digest,
            derivation_artifact_id: roles.derivation_contract,
            derivation_logical_digest: derivation.logical_digest(),
            output_dimension: self.output_dimension(),
        })
    }
}

pub(super) fn require_patch_region_profile(
    record: &ArtifactRecord,
    row_count: u64,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    require_physical_common(record, MultiscaleEmbeddingArtifactRole::PatchRegionLink)?;
    if !record_matches(record, SpatialArtifactRole::PatchRegion, row_count) {
        return Err(
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            },
        );
    }
    Ok(())
}

fn require_patch_table_profile(
    record: &ArtifactRecord,
    row_count: u64,
    dimension: u32,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    require_physical_common(record, MultiscaleEmbeddingArtifactRole::SourcePatchTable)?;
    let profile = MatrixPhysicalProfile::Patch;
    let matches = [
        SpatialPhysicalEncoding::Arrow,
        SpatialPhysicalEncoding::Parquet,
    ]
    .into_iter()
    .any(|encoding| profile.record_matches_encoding(record, encoding, row_count, dimension));
    if !matches {
        return Err(
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            },
        );
    }
    Ok(())
}

fn require_physical_common(
    record: &ArtifactRecord,
    role: MultiscaleEmbeddingArtifactRole,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    let (schema, content_kind_matches) = match role {
        MultiscaleEmbeddingArtifactRole::SourcePatchTable => {
            let profile = MatrixPhysicalProfile::Patch;
            (
                "marklab.patch_embedding_table",
                [
                    SpatialPhysicalEncoding::Arrow,
                    SpatialPhysicalEncoding::Parquet,
                ]
                .into_iter()
                .any(|encoding| record.content().kind() == profile.content_kind(encoding)),
            )
        }
        MultiscaleEmbeddingArtifactRole::PatchRegionLink => (
            "marklab.patch_region_link",
            [
                SpatialPhysicalEncoding::Arrow,
                SpatialPhysicalEncoding::Parquet,
            ]
            .into_iter()
            .any(|encoding| {
                record.content().kind()
                    == spatial_content_kind(SpatialArtifactRole::PatchRegion, encoding)
            }),
        ),
        _ => return binding_mismatch(role),
    };
    if record.schema().id() != schema || record.schema().version() != 1 {
        return Err(MultiscaleEmbeddingArtifactGraphError::SchemaMismatch { role });
    }
    if !content_kind_matches {
        return Err(MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch { role });
    }
    if !record.semantic_metadata().is_empty() {
        return Err(MultiscaleEmbeddingArtifactGraphError::SemanticMetadataMismatch { role });
    }
    if record.table().is_none() {
        return Err(MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch { role });
    }
    Ok(())
}

fn require_distinct_roles(
    roles: &[(MultiscaleEmbeddingArtifactRole, ArtifactId); 9],
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

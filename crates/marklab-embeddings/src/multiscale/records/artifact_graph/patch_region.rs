use std::{convert::Infallible, fmt};

use marklab_project::{
    ArtifactCatalog, ArtifactId, ArtifactRecord, ContentDigest, LocalArtifactStore,
    VerifiedReaderError,
};
use thiserror::Error;

use super::managed::availability_failure;
use crate::{
    columnar::{
        validate_patch_footprint_set_arrow_from_store,
        validate_patch_footprint_set_parquet_from_store, EmbeddingColumnarBudgets,
        MultiscaleColumnarError,
    },
    multiscale::{
        context::PatchEmbeddingContext,
        expected::{ExpectedPatchSet, ExpectedRegionSet},
        footprint::PatchFootprintSet,
        json::CanonicalJsonReaderError,
        patch_region::{PatchRegionAssessment, PatchRegionLink},
        physical::{record_matches, SpatialArtifactRole},
    },
    ArtifactAvailabilityFailure,
};

/// Artifact roles admitted by vector-independent patch-region input validation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum PatchRegionInputArtifactRole {
    /// The exact expected-patch set.
    ExpectedPatches,
    /// The exact expected-region set.
    ExpectedRegions,
    /// The calibrated patch context.
    PatchContext,
    /// The fully decoded patch-footprint artifact.
    PatchFootprints,
    /// The converter manifest that produced the exhaustive assessment.
    Converter,
    /// The canonical producer-declared exhaustive assessment.
    Assessment,
}

impl fmt::Display for PatchRegionInputArtifactRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExpectedPatches => "expected patches",
            Self::ExpectedRegions => "expected regions",
            Self::PatchContext => "patch context",
            Self::PatchFootprints => "patch footprints",
            Self::Converter => "converter",
            Self::Assessment => "patch-region assessment",
        })
    }
}

/// Exact patch-region input record, payload, physical, binding, or availability failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[non_exhaustive]
pub enum PatchRegionInputArtifactGraphError {
    /// A required record is absent from the catalog.
    #[error("required {role} artifact is absent from the catalog")]
    MissingRecord {
        /// Missing role.
        role: PatchRegionInputArtifactRole,
    },
    /// A required record has the wrong schema ID or version.
    #[error("required {role} artifact has the wrong schema or version")]
    SchemaMismatch {
        /// Mismatched role.
        role: PatchRegionInputArtifactRole,
    },
    /// A required record has the wrong exact media kind.
    #[error("required {role} artifact has the wrong content kind")]
    ContentKindMismatch {
        /// Mismatched role.
        role: PatchRegionInputArtifactRole,
    },
    /// A non-table role unexpectedly declares a table manifest.
    #[error("required {role} artifact has an unexpected table manifest")]
    UnexpectedTableManifest {
        /// Mismatched role.
        role: PatchRegionInputArtifactRole,
    },
    /// The footprint table manifest differs from its exact physical profile.
    #[error("required patch footprints artifact has the wrong table manifest")]
    TableManifestMismatch,
    /// A required artifact carries forbidden semantic metadata.
    #[error("required {role} artifact semantic metadata must be empty")]
    SemanticMetadataMismatch {
        /// Mismatched role.
        role: PatchRegionInputArtifactRole,
    },
    /// Direct dependencies differ from the exact frozen role set.
    #[error("required {role} artifact has the wrong direct dependencies")]
    DependencyMismatch {
        /// Mismatched role.
        role: PatchRegionInputArtifactRole,
    },
    /// Two distinct patch-region roles unexpectedly name one artifact.
    #[error("patch-region input artifact roles must be distinct")]
    RoleAlias,
    /// Decoded domain values disagree about one exact role binding.
    #[error("decoded patch-region values disagree about the {role} binding")]
    DomainBindingMismatch {
        /// Inconsistent role.
        role: PatchRegionInputArtifactRole,
    },
    /// Canonical domain bytes differ from the referenced artifact content.
    #[error("required {role} artifact does not match the canonical domain payload")]
    PayloadIdentityMismatch {
        /// Mismatched role.
        role: PatchRegionInputArtifactRole,
    },
    /// A catalog record cannot be verified through the managed store.
    #[error("required {role} artifact is unavailable from the managed store: {reason:?}")]
    Unavailable {
        /// Unavailable role.
        role: PatchRegionInputArtifactRole,
        /// Redacted failure category.
        reason: ArtifactAvailabilityFailure,
    },
    /// The footprint artifact failed its exact physical decode.
    #[error("patch footprint physical validation failed: {reason}")]
    FootprintPhysical {
        /// Redacted physical failure.
        reason: MultiscaleColumnarError,
    },
}

/// Runtime-only proof of the exact vector-independent patch-region input graph.
///
/// This proves a producer-declared exhaustive assessment and its physical patch-support inputs. It
/// does not prove region geometry, coordinate correspondence, vector content, or real-corpus
/// promotion.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedPatchRegionInputArtifactGraph {
    pub(crate) expected_patches_artifact_id: ArtifactId,
    pub(crate) expected_regions_artifact_id: ArtifactId,
    pub(crate) patch_context_artifact_id: ArtifactId,
    pub(crate) patch_footprints_artifact_id: ArtifactId,
    pub(crate) converter_artifact_id: ArtifactId,
    pub(crate) converter_content_digest: ContentDigest,
    pub(crate) assessment_artifact_id: ArtifactId,
    pub(crate) assessment_content_digest: ContentDigest,
    pub(crate) link_logical_digest: ContentDigest,
    pub(crate) assessed_pair_count: u64,
    pub(crate) nonzero_relation_count: u64,
}

impl fmt::Debug for VerifiedPatchRegionInputArtifactGraph {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedPatchRegionInputArtifactGraph")
            .field("assessed_pair_count", &self.assessed_pair_count)
            .field("nonzero_relation_count", &self.nonzero_relation_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedPatchRegionInputArtifactGraph {
    /// Exact producer-declared Cartesian pair count.
    pub fn assessed_pair_count(self) -> u64 {
        self.assessed_pair_count
    }

    /// Exact stored nonzero relation count.
    pub fn nonzero_relation_count(self) -> u64 {
        self.nonzero_relation_count
    }

    /// Format-independent patch-region link identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.link_logical_digest
    }

    /// Exact exhaustive-assessment artifact identity.
    pub fn assessment_artifact_id(self) -> ArtifactId {
        self.assessment_artifact_id
    }
}

impl PatchRegionAssessment {
    /// Validate the vector-independent patch-region input graph and fully decode its footprint.
    ///
    /// # Errors
    ///
    /// Returns a role-only error for record, dependency, payload, binding, physical footprint, or
    /// managed availability drift. This never establishes geometric truth for declared overlap.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_region_input_artifact_graph(
        &self,
        assessment_artifact_id: ArtifactId,
        expected_patches: &ExpectedPatchSet,
        expected_regions: &ExpectedRegionSet,
        context: &PatchEmbeddingContext,
        footprints: &PatchFootprintSet,
        link: &PatchRegionLink,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
        budgets: EmbeddingColumnarBudgets,
    ) -> Result<VerifiedPatchRegionInputArtifactGraph, PatchRegionInputArtifactGraphError> {
        validate_domain_bindings(
            self,
            assessment_artifact_id,
            expected_patches,
            expected_regions,
            context,
            footprints,
            link,
        )?;
        let ids = role_ids(link);
        validate_distinct_roles(&ids)?;
        let footprint_rows = u64::try_from(footprints.row_count())
            .map_err(|_| PatchRegionInputArtifactGraphError::TableManifestMismatch)?;
        for (role, id) in ids {
            require_profile(required_record(catalog, role, id)?, role, footprint_rows)?;
        }
        let converter_record = required_record(
            catalog,
            PatchRegionInputArtifactRole::Converter,
            link.converter_artifact_id(),
        )?;
        if converter_record.content().digest() != link.converter_content_digest() {
            return binding_mismatch(PatchRegionInputArtifactRole::Converter);
        }
        let assessment_record = required_record(
            catalog,
            PatchRegionInputArtifactRole::Assessment,
            assessment_artifact_id,
        )?;
        if assessment_record.content().digest() != link.assessment_content_digest() {
            return binding_mismatch(PatchRegionInputArtifactRole::Assessment);
        }
        validate_dependencies(link, catalog)?;
        require_json_payload(
            store,
            required_record(
                catalog,
                PatchRegionInputArtifactRole::ExpectedPatches,
                link.expected_patches_artifact_id(),
            )?,
            PatchRegionInputArtifactRole::ExpectedPatches,
            |reader| expected_patches.compare_canonical_json_reader(reader),
        )?;
        require_json_payload(
            store,
            required_record(
                catalog,
                PatchRegionInputArtifactRole::ExpectedRegions,
                link.expected_regions_artifact_id(),
            )?,
            PatchRegionInputArtifactRole::ExpectedRegions,
            |reader| expected_regions.compare_canonical_json_reader(reader),
        )?;
        require_json_payload(
            store,
            required_record(
                catalog,
                PatchRegionInputArtifactRole::PatchContext,
                link.patch_context_artifact_id(),
            )?,
            PatchRegionInputArtifactRole::PatchContext,
            |reader| context.compare_canonical_json_reader(reader),
        )?;
        require_json_payload(
            store,
            assessment_record,
            PatchRegionInputArtifactRole::Assessment,
            |reader| self.compare_canonical_json_reader(reader),
        )?;
        require_available(
            store,
            converter_record,
            PatchRegionInputArtifactRole::Converter,
        )?;
        let footprint_record = required_record(
            catalog,
            PatchRegionInputArtifactRole::PatchFootprints,
            link.patch_footprints_artifact_id(),
        )?;
        validate_physical_footprints(
            store,
            footprint_record,
            expected_patches,
            context,
            footprints,
            budgets,
        )?;
        Ok(VerifiedPatchRegionInputArtifactGraph {
            expected_patches_artifact_id: link.expected_patches_artifact_id(),
            expected_regions_artifact_id: link.expected_regions_artifact_id(),
            patch_context_artifact_id: link.patch_context_artifact_id(),
            patch_footprints_artifact_id: link.patch_footprints_artifact_id(),
            converter_artifact_id: link.converter_artifact_id(),
            converter_content_digest: link.converter_content_digest(),
            assessment_artifact_id,
            assessment_content_digest: link.assessment_content_digest(),
            link_logical_digest: link.logical_digest(),
            assessed_pair_count: link.assessed_pair_count(),
            nonzero_relation_count: u64::try_from(link.nonzero_relation_count()).map_err(|_| {
                PatchRegionInputArtifactGraphError::DomainBindingMismatch {
                    role: PatchRegionInputArtifactRole::Assessment,
                }
            })?,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_domain_bindings(
    assessment: &PatchRegionAssessment,
    assessment_artifact_id: ArtifactId,
    expected_patches: &ExpectedPatchSet,
    expected_regions: &ExpectedRegionSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    link: &PatchRegionLink,
) -> Result<(), PatchRegionInputArtifactGraphError> {
    if link.expected_patches_logical_digest() != expected_patches.logical_digest()
        || link.expected_patches_artifact_id() != footprints.expected_patches_artifact_id()
        || footprints.expected_patches_logical_digest() != expected_patches.logical_digest()
        || expected_patches.owning_slide_id() != link.owning_slide_id()
    {
        return binding_mismatch(PatchRegionInputArtifactRole::ExpectedPatches);
    }
    if link.expected_regions_logical_digest() != expected_regions.logical_digest()
        || expected_regions.owning_slide_id() != link.owning_slide_id()
    {
        return binding_mismatch(PatchRegionInputArtifactRole::ExpectedRegions);
    }
    if link.patch_context_artifact_id() != footprints.patch_context_artifact_id()
        || link.patch_context_logical_digest() != context.logical_digest()
        || footprints.patch_context_logical_digest() != context.logical_digest()
        || context.owning_slide_id() != link.owning_slide_id()
    {
        return binding_mismatch(PatchRegionInputArtifactRole::PatchContext);
    }
    if link.patch_footprints_logical_digest() != footprints.logical_digest() {
        return binding_mismatch(PatchRegionInputArtifactRole::PatchFootprints);
    }
    if link.assessment_artifact_id() != assessment_artifact_id
        || assessment.owning_slide_id() != link.owning_slide_id()
        || assessment.assessed_pair_count() != link.assessed_pair_count()
        || assessment.nonzero_relations() != link.nonzero_relations()
    {
        return binding_mismatch(PatchRegionInputArtifactRole::Assessment);
    }
    let mut expected_dependencies = [
        link.expected_patches_artifact_id(),
        link.expected_regions_artifact_id(),
        link.patch_context_artifact_id(),
        link.patch_footprints_artifact_id(),
        link.converter_artifact_id(),
    ];
    expected_dependencies.sort_unstable();
    if !assessment.direct_dependencies().eq(expected_dependencies) {
        return binding_mismatch(PatchRegionInputArtifactRole::Assessment);
    }
    Ok(())
}

fn role_ids(link: &PatchRegionLink) -> [(PatchRegionInputArtifactRole, ArtifactId); 6] {
    [
        (
            PatchRegionInputArtifactRole::ExpectedPatches,
            link.expected_patches_artifact_id(),
        ),
        (
            PatchRegionInputArtifactRole::ExpectedRegions,
            link.expected_regions_artifact_id(),
        ),
        (
            PatchRegionInputArtifactRole::PatchContext,
            link.patch_context_artifact_id(),
        ),
        (
            PatchRegionInputArtifactRole::PatchFootprints,
            link.patch_footprints_artifact_id(),
        ),
        (
            PatchRegionInputArtifactRole::Converter,
            link.converter_artifact_id(),
        ),
        (
            PatchRegionInputArtifactRole::Assessment,
            link.assessment_artifact_id(),
        ),
    ]
}

fn validate_distinct_roles(
    roles: &[(PatchRegionInputArtifactRole, ArtifactId); 6],
) -> Result<(), PatchRegionInputArtifactGraphError> {
    let mut ids = roles.map(|(_, id)| id);
    ids.sort_unstable();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(PatchRegionInputArtifactGraphError::RoleAlias);
    }
    Ok(())
}

fn required_record(
    catalog: &ArtifactCatalog,
    role: PatchRegionInputArtifactRole,
    id: ArtifactId,
) -> Result<&ArtifactRecord, PatchRegionInputArtifactGraphError> {
    catalog
        .get(id)
        .ok_or(PatchRegionInputArtifactGraphError::MissingRecord { role })
}

fn require_profile(
    record: &ArtifactRecord,
    role: PatchRegionInputArtifactRole,
    footprint_rows: u64,
) -> Result<(), PatchRegionInputArtifactGraphError> {
    if record.schema().id() != schema_id(role) || record.schema().version() != 1 {
        return Err(PatchRegionInputArtifactGraphError::SchemaMismatch { role });
    }
    if !record.semantic_metadata().is_empty() {
        return Err(PatchRegionInputArtifactGraphError::SemanticMetadataMismatch { role });
    }
    if role == PatchRegionInputArtifactRole::PatchFootprints {
        if !matches!(
            record.content().kind(),
            "application/vnd.marklab.patch-footprint-table.v1+arrow"
                | "application/vnd.marklab.patch-footprint-table.v1+parquet"
        ) {
            return Err(PatchRegionInputArtifactGraphError::ContentKindMismatch { role });
        }
        if !record_matches(record, SpatialArtifactRole::Footprint, footprint_rows) {
            return Err(PatchRegionInputArtifactGraphError::TableManifestMismatch);
        }
    } else {
        if record.content().kind() != content_kind(role) {
            return Err(PatchRegionInputArtifactGraphError::ContentKindMismatch { role });
        }
        if record.table().is_some() {
            return Err(PatchRegionInputArtifactGraphError::UnexpectedTableManifest { role });
        }
    }
    Ok(())
}

fn schema_id(role: PatchRegionInputArtifactRole) -> &'static str {
    match role {
        PatchRegionInputArtifactRole::ExpectedPatches => "marklab.expected_patch_set",
        PatchRegionInputArtifactRole::ExpectedRegions => "marklab.expected_region_set",
        PatchRegionInputArtifactRole::PatchContext => "marklab.patch_embedding_context",
        PatchRegionInputArtifactRole::PatchFootprints => "marklab.patch_footprint_table",
        PatchRegionInputArtifactRole::Converter => "marklab.converter_manifest",
        PatchRegionInputArtifactRole::Assessment => "marklab.patch_region_assessment",
    }
}

fn content_kind(role: PatchRegionInputArtifactRole) -> &'static str {
    match role {
        PatchRegionInputArtifactRole::ExpectedPatches => {
            "application/vnd.marklab.expected-patch-set.v1+json"
        }
        PatchRegionInputArtifactRole::ExpectedRegions => {
            "application/vnd.marklab.expected-region-set.v1+json"
        }
        PatchRegionInputArtifactRole::PatchContext => {
            "application/vnd.marklab.patch-embedding-context.v1+json"
        }
        PatchRegionInputArtifactRole::Converter => "application/json",
        PatchRegionInputArtifactRole::Assessment => {
            "application/vnd.marklab.patch-region-assessment.v1+json"
        }
        PatchRegionInputArtifactRole::PatchFootprints => "",
    }
}

fn validate_dependencies(
    link: &PatchRegionLink,
    catalog: &ArtifactCatalog,
) -> Result<(), PatchRegionInputArtifactGraphError> {
    for (role, id) in [
        (
            PatchRegionInputArtifactRole::ExpectedPatches,
            link.expected_patches_artifact_id(),
        ),
        (
            PatchRegionInputArtifactRole::ExpectedRegions,
            link.expected_regions_artifact_id(),
        ),
        (
            PatchRegionInputArtifactRole::PatchContext,
            link.patch_context_artifact_id(),
        ),
        (
            PatchRegionInputArtifactRole::Converter,
            link.converter_artifact_id(),
        ),
    ] {
        require_dependencies(required_record(catalog, role, id)?, role, &[])?;
    }
    let mut footprint_dependencies = [
        link.expected_patches_artifact_id(),
        link.patch_context_artifact_id(),
    ];
    footprint_dependencies.sort_unstable();
    require_dependencies(
        required_record(
            catalog,
            PatchRegionInputArtifactRole::PatchFootprints,
            link.patch_footprints_artifact_id(),
        )?,
        PatchRegionInputArtifactRole::PatchFootprints,
        &footprint_dependencies,
    )?;
    let mut expected = [
        link.expected_patches_artifact_id(),
        link.expected_regions_artifact_id(),
        link.patch_context_artifact_id(),
        link.patch_footprints_artifact_id(),
        link.converter_artifact_id(),
    ];
    expected.sort_unstable();
    require_dependencies(
        required_record(
            catalog,
            PatchRegionInputArtifactRole::Assessment,
            link.assessment_artifact_id(),
        )?,
        PatchRegionInputArtifactRole::Assessment,
        &expected,
    )
}

fn require_dependencies(
    record: &ArtifactRecord,
    role: PatchRegionInputArtifactRole,
    expected: &[ArtifactId],
) -> Result<(), PatchRegionInputArtifactGraphError> {
    if record.dependencies() != expected {
        return Err(PatchRegionInputArtifactGraphError::DependencyMismatch { role });
    }
    Ok(())
}

fn require_json_payload<F>(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    role: PatchRegionInputArtifactRole,
    compare: F,
) -> Result<(), PatchRegionInputArtifactGraphError>
where
    F: FnOnce(&mut dyn marklab_project::ArtifactReadSeek) -> Result<(), CanonicalJsonReaderError>,
{
    store
        .with_verified_reader(record, compare)
        .map_err(|error| match error {
            VerifiedReaderError::Store(error) => PatchRegionInputArtifactGraphError::Unavailable {
                role,
                reason: availability_failure(&error),
            },
            VerifiedReaderError::Callback(CanonicalJsonReaderError::Mismatch) => {
                PatchRegionInputArtifactGraphError::PayloadIdentityMismatch { role }
            }
            VerifiedReaderError::Callback(CanonicalJsonReaderError::Read) => {
                PatchRegionInputArtifactGraphError::Unavailable {
                    role,
                    reason: ArtifactAvailabilityFailure::StoreAccess,
                }
            }
        })
}

fn require_available(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    role: PatchRegionInputArtifactRole,
) -> Result<(), PatchRegionInputArtifactGraphError> {
    store
        .with_verified_reader(record, |_reader| Ok::<(), Infallible>(()))
        .map_err(|error| match error {
            VerifiedReaderError::Store(error) => PatchRegionInputArtifactGraphError::Unavailable {
                role,
                reason: availability_failure(&error),
            },
            VerifiedReaderError::Callback(error) => match error {},
        })
}

fn validate_physical_footprints(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), PatchRegionInputArtifactGraphError> {
    let result = match record.content().kind() {
        "application/vnd.marklab.patch-footprint-table.v1+arrow" => {
            validate_patch_footprint_set_arrow_from_store(
                store, record, expected, context, footprints, budgets,
            )
            .map(|_| ())
        }
        "application/vnd.marklab.patch-footprint-table.v1+parquet" => {
            validate_patch_footprint_set_parquet_from_store(
                store, record, expected, context, footprints, budgets,
            )
            .map(|_| ())
        }
        _ => {
            return Err(PatchRegionInputArtifactGraphError::ContentKindMismatch {
                role: PatchRegionInputArtifactRole::PatchFootprints,
            })
        }
    };
    result.map_err(|error| match error {
        VerifiedReaderError::Store(error) => PatchRegionInputArtifactGraphError::Unavailable {
            role: PatchRegionInputArtifactRole::PatchFootprints,
            reason: availability_failure(&error),
        },
        VerifiedReaderError::Callback(reason) => {
            PatchRegionInputArtifactGraphError::FootprintPhysical { reason }
        }
    })
}

fn binding_mismatch<T>(
    role: PatchRegionInputArtifactRole,
) -> Result<T, PatchRegionInputArtifactGraphError> {
    Err(PatchRegionInputArtifactGraphError::DomainBindingMismatch { role })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use marklab_project::{ArtifactId, ContentDigest};

    use super::{
        validate_distinct_roles, PatchRegionInputArtifactGraphError, PatchRegionInputArtifactRole,
    };

    #[test]
    fn patch_region_input_roles_reject_every_alias() {
        let id = ArtifactId::from_str(&ContentDigest::from_bytes(b"aliased-role").to_string())
            .expect("artifact ID");
        let roles = [
            (PatchRegionInputArtifactRole::ExpectedPatches, id),
            (PatchRegionInputArtifactRole::ExpectedRegions, id),
            (PatchRegionInputArtifactRole::PatchContext, id),
            (PatchRegionInputArtifactRole::PatchFootprints, id),
            (PatchRegionInputArtifactRole::Converter, id),
            (PatchRegionInputArtifactRole::Assessment, id),
        ];
        assert_eq!(
            validate_distinct_roles(&roles),
            Err(PatchRegionInputArtifactGraphError::RoleAlias)
        );
    }
}

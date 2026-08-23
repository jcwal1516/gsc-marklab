use std::fmt;

use marklab_project::{ArtifactId, ContentDigest};
use thiserror::Error;

use crate::ArtifactAvailabilityFailure;

/// Artifact roles admitted by direct-patch structural graph validation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum MultiscaleEmbeddingArtifactRole {
    /// The strict multiscale provenance document.
    Provenance,
    /// The exact model checkpoint bytes.
    Checkpoint,
    /// The reviewed source snapshot.
    SourceSnapshot,
    /// The explicit license record.
    LicenseRecord,
    /// The exact input-normalization contract.
    InputNormalization,
    /// The preprocessing declaration.
    Preprocessing,
    /// The inference run configuration.
    RunConfig,
    /// The execution environment declaration.
    Environment,
    /// The converter manifest.
    Converter,
    /// The source-local patch entity universe.
    SourceEntities,
    /// The opaque source-vector bytes.
    SourceVectors,
    /// The canonical expected-patch set.
    ExpectedPatches,
    /// The source-to-patch identity map.
    IdentityMap,
    /// The exact source-row correspondence document.
    SourceRowLink,
    /// The calibrated patch extraction context.
    PatchContext,
    /// The patch-footprint physical artifact.
    PatchFootprints,
    /// The patch-overlap physical artifact.
    PatchOverlapGraph,
    /// The patch support descriptor.
    PatchSupport,
}

impl fmt::Display for MultiscaleEmbeddingArtifactRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Provenance => "provenance",
            Self::Checkpoint => "checkpoint",
            Self::SourceSnapshot => "source snapshot",
            Self::LicenseRecord => "license record",
            Self::InputNormalization => "input normalization",
            Self::Preprocessing => "preprocessing",
            Self::RunConfig => "run config",
            Self::Environment => "environment",
            Self::Converter => "converter",
            Self::SourceEntities => "source entities",
            Self::SourceVectors => "source vectors",
            Self::ExpectedPatches => "expected patches",
            Self::IdentityMap => "identity map",
            Self::SourceRowLink => "source row link",
            Self::PatchContext => "patch context",
            Self::PatchFootprints => "patch footprints",
            Self::PatchOverlapGraph => "patch overlap graph",
            Self::PatchSupport => "patch support",
        })
    }
}

/// Exact structural graph, record-profile, binding, payload, or availability failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[non_exhaustive]
pub enum MultiscaleEmbeddingArtifactGraphError {
    /// The supplied provenance is not direct-patch provenance.
    #[error("direct-patch artifact graph validation requires direct-patch provenance")]
    UnsupportedProvenanceVariant,
    /// A required schema-bound artifact is absent from the catalog.
    #[error("required {role} artifact is absent from the catalog")]
    MissingRecord {
        /// Missing artifact role.
        role: MultiscaleEmbeddingArtifactRole,
    },
    /// A required artifact has the wrong schema ID or version.
    #[error("required {role} artifact has the wrong schema or version")]
    SchemaMismatch {
        /// Mismatched artifact role.
        role: MultiscaleEmbeddingArtifactRole,
    },
    /// A required artifact has the wrong exact media kind.
    #[error("required {role} artifact has the wrong content kind")]
    ContentKindMismatch {
        /// Mismatched artifact role.
        role: MultiscaleEmbeddingArtifactRole,
    },
    /// A non-table role unexpectedly declares a table manifest.
    #[error("required {role} artifact has an unexpected table manifest")]
    UnexpectedTableManifest {
        /// Mismatched artifact role.
        role: MultiscaleEmbeddingArtifactRole,
    },
    /// A physical role's table declaration differs from its exact profile.
    #[error("required {role} artifact has the wrong table manifest")]
    TableManifestMismatch {
        /// Mismatched artifact role.
        role: MultiscaleEmbeddingArtifactRole,
    },
    /// A required artifact carries forbidden semantic metadata.
    #[error("required {role} artifact semantic metadata must be empty")]
    SemanticMetadataMismatch {
        /// Mismatched artifact role.
        role: MultiscaleEmbeddingArtifactRole,
    },
    /// Direct dependencies differ from the exact frozen role set.
    #[error("required {role} artifact has the wrong direct dependencies")]
    DependencyMismatch {
        /// Mismatched artifact role.
        role: MultiscaleEmbeddingArtifactRole,
    },
    /// Decoded domain values disagree about one artifact role or logical identity.
    #[error("decoded domain values disagree about the {role} binding")]
    DomainBindingMismatch {
        /// Inconsistent artifact role.
        role: MultiscaleEmbeddingArtifactRole,
    },
    /// Canonical domain bytes differ from the referenced artifact content.
    #[error("required {role} artifact does not match the canonical domain payload")]
    PayloadIdentityMismatch {
        /// Mismatched artifact role.
        role: MultiscaleEmbeddingArtifactRole,
    },
    /// The checkpoint digest declared by provenance differs from the checkpoint content digest.
    #[error("checkpoint content digest does not match provenance")]
    CheckpointDigestMismatch,
    /// A catalog record cannot be verified through the managed store.
    #[error("required {role} artifact is unavailable from the managed store: {reason:?}")]
    Unavailable {
        /// Unavailable artifact role.
        role: MultiscaleEmbeddingArtifactRole,
        /// Redacted failure category.
        reason: ArtifactAvailabilityFailure,
    },
}

/// Runtime-only proof of the exact direct-patch structural artifact graph.
///
/// This proves record schemas, kinds, table manifests, dependencies, canonical JSON payloads,
/// cross-value bindings, and managed content integrity. It does not prove that footprint or
/// overlap bytes decode, that source vectors follow an adapter grammar, or that any physical
/// embedding table is promotable.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedDirectPatchEmbeddingArtifactGraph {
    pub(crate) provenance_artifact_id: ArtifactId,
    pub(crate) provenance_dependency_count: u8,
    pub(crate) source_entities_artifact_id: ArtifactId,
    pub(crate) source_vectors_artifact_id: ArtifactId,
    pub(crate) expected_patches_artifact_id: ArtifactId,
    pub(crate) expected_patches_logical_digest: ContentDigest,
    pub(crate) identity_map_artifact_id: ArtifactId,
    pub(crate) source_row_link_artifact_id: ArtifactId,
    pub(crate) source_row_link_logical_digest: ContentDigest,
    pub(crate) patch_support_artifact_id: ArtifactId,
    pub(crate) patch_support_logical_digest: ContentDigest,
    pub(crate) converter_artifact_id: ArtifactId,
    pub(crate) output_dimension: u32,
}

impl fmt::Debug for VerifiedDirectPatchEmbeddingArtifactGraph {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedDirectPatchEmbeddingArtifactGraph")
            .field(
                "provenance_dependency_count",
                &self.provenance_dependency_count,
            )
            .field("output_dimension", &self.output_dimension)
            .finish_non_exhaustive()
    }
}

impl VerifiedDirectPatchEmbeddingArtifactGraph {
    /// Verified provenance artifact identity.
    pub fn provenance_artifact_id(self) -> ArtifactId {
        self.provenance_artifact_id
    }

    /// Number of exact direct provenance dependencies.
    pub fn provenance_dependency_count(self) -> u8 {
        self.provenance_dependency_count
    }

    /// Provenance-declared positive output dimension.
    pub fn output_dimension(self) -> u32 {
        self.output_dimension
    }
}

use std::fmt;

use marklab_project::{ArtifactId, ContentDigest};
use thiserror::Error;

/// Closed artifact role vocabulary for embedding promotion validation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CellEmbeddingArtifactRole {
    /// The strict canonical embedding provenance artifact.
    Provenance,
    /// The exact model checkpoint bytes.
    Checkpoint,
    /// The reviewed model-source snapshot.
    SourceSnapshot,
    /// The explicit license record.
    LicenseRecord,
    /// The preprocessing and stain-normalization declaration.
    Preprocessing,
    /// The inference run configuration.
    RunConfig,
    /// The execution environment declaration.
    Environment,
    /// The converter manifest.
    Converter,
    /// The source cell metadata table.
    SourceCells,
    /// The source embedding tensor.
    SourceVectors,
    /// The canonical expected-cell set.
    ExpectedCells,
    /// The explicit source-to-canonical identity map.
    IdentityMap,
    /// The calibrated spatial extraction context.
    SpatialContext,
    /// The exact source-row correspondence table.
    RowLink,
}

impl fmt::Display for CellEmbeddingArtifactRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Provenance => "provenance",
            Self::Checkpoint => "checkpoint",
            Self::SourceSnapshot => "source snapshot",
            Self::LicenseRecord => "license record",
            Self::Preprocessing => "preprocessing",
            Self::RunConfig => "run config",
            Self::Environment => "environment",
            Self::Converter => "converter",
            Self::SourceCells => "source cells",
            Self::SourceVectors => "source vectors",
            Self::ExpectedCells => "expected cells",
            Self::IdentityMap => "identity map",
            Self::SpatialContext => "spatial context",
            Self::RowLink => "row link",
        })
    }
}

/// Privacy-safe category for one failed managed-store verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactAvailabilityFailure {
    /// The record has no managed replica for the selected store.
    LocatorMissing,
    /// The managed object is absent.
    ObjectMissing,
    /// The managed bytes differ from the declared digest or length.
    Integrity,
    /// The object crosses a symlink or is not a supported regular file.
    UnsupportedFileType,
    /// A capability-relative store operation failed.
    StoreAccess,
    /// A store invariant or unsupported operation failed.
    StoreInvariant,
}

/// Exact graph, schema, payload-binding, or availability validation failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum EmbeddingArtifactGraphError {
    /// A required schema-bound artifact is absent from the catalog.
    #[error("required {role} artifact is absent from the catalog")]
    MissingRecord {
        /// Missing artifact role.
        role: CellEmbeddingArtifactRole,
    },
    /// A required artifact has the wrong schema ID or version.
    #[error("required {role} artifact has the wrong schema or version")]
    SchemaMismatch {
        /// Mismatched artifact role.
        role: CellEmbeddingArtifactRole,
    },
    /// A C-04 payload has the wrong exact media kind.
    #[error("required {role} artifact has the wrong content kind")]
    ContentKindMismatch {
        /// Mismatched artifact role.
        role: CellEmbeddingArtifactRole,
    },
    /// A non-table C-04 artifact unexpectedly declares a table manifest.
    #[error("required {role} artifact has an unexpected table manifest")]
    UnexpectedTableManifest {
        /// Mismatched artifact role.
        role: CellEmbeddingArtifactRole,
    },
    /// The row-link table declaration differs from the frozen profile.
    #[error("row-link artifact has the wrong table manifest")]
    RowLinkManifestMismatch,
    /// A C-04 artifact carries forbidden semantic metadata.
    #[error("required {role} artifact semantic metadata must be empty")]
    SemanticMetadataMismatch {
        /// Mismatched artifact role.
        role: CellEmbeddingArtifactRole,
    },
    /// Direct dependencies differ from the exact frozen graph.
    #[error("required {role} artifact has the wrong direct dependencies")]
    DependencyMismatch {
        /// Mismatched artifact role.
        role: CellEmbeddingArtifactRole,
    },
    /// Decoded domain values name inconsistent artifact roles.
    #[error("embedding domain values disagree about artifact linkage")]
    LinkageMismatch,
    /// Canonical domain bytes do not match the referenced artifact content identity.
    #[error("required {role} artifact does not match the canonical domain payload")]
    PayloadIdentityMismatch {
        /// Mismatched artifact role.
        role: CellEmbeddingArtifactRole,
    },
    /// Checkpoint provenance does not match the checkpoint record's content digest.
    #[error("checkpoint content digest does not match provenance")]
    CheckpointDigestMismatch,
    /// A catalog record cannot be verified through the managed local store.
    #[error("required {role} artifact is unavailable from the managed store: {reason:?}")]
    Unavailable {
        /// Unavailable artifact role.
        role: CellEmbeddingArtifactRole,
        /// Redacted failure category.
        reason: ArtifactAvailabilityFailure,
    },
}

/// Evidence that the complete provenance graph and every managed replica were verified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCellEmbeddingArtifactGraph {
    pub(crate) provenance_artifact_id: ArtifactId,
    pub(crate) dependency_count: u8,
    pub(crate) source_cells_artifact_id: ArtifactId,
    pub(crate) source_vectors_artifact_id: ArtifactId,
    pub(crate) expected_cells_artifact_id: ArtifactId,
    pub(crate) identity_map_artifact_id: ArtifactId,
    pub(crate) converter_artifact_id: ArtifactId,
    pub(crate) row_link_artifact_id: ArtifactId,
    pub(crate) expected_cells_logical_digest: ContentDigest,
    pub(crate) row_link_logical_digest: ContentDigest,
}

impl VerifiedCellEmbeddingArtifactGraph {
    /// Verified provenance artifact identity.
    pub fn provenance_artifact_id(self) -> ArtifactId {
        self.provenance_artifact_id
    }

    /// Number of verified direct provenance dependencies.
    pub fn dependency_count(self) -> u8 {
        self.dependency_count
    }
}

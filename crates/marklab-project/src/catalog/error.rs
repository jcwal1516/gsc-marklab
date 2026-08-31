use super::*;

/// Strict catalog decode, dependency, identity, and replica-merge failures.
#[derive(Debug, Error)]
pub enum ArtifactCatalogError {
    /// Input or encoded catalog exceeds the fixed byte bound.
    #[error("artifact catalog has {observed} bytes; maximum is {maximum}")]
    CatalogTooLarge {
        /// Observed size.
        observed: usize,
        /// Maximum size.
        maximum: usize,
    },
    /// Catalog has too many unique artifacts.
    #[error("artifact catalog has {observed} artifacts; maximum is {maximum}")]
    TooManyArtifacts {
        /// Observed count.
        observed: usize,
        /// Maximum count.
        maximum: usize,
    },
    /// JSON is malformed, contains an unknown/duplicate field, or cannot encode.
    #[error("invalid artifact catalog JSON: {reason}")]
    Json {
        /// Parser/encoder context.
        reason: String,
    },
    /// Top-level format identifier is not the C-03 catalog format.
    #[error("unsupported artifact catalog format {observed:?}")]
    InvalidCatalogFormat {
        /// Observed format.
        observed: String,
    },
    /// Catalog version is old, zero, or from the future.
    #[error("unsupported artifact catalog version {observed}")]
    UnsupportedCatalogVersion {
        /// Observed version.
        observed: u32,
    },
    /// Digest text is not canonical lowercase SHA-256 hex.
    #[error("invalid digest {value:?}: {source}")]
    InvalidDigest {
        /// Rejected digest text.
        value: String,
        /// Strict parser error.
        #[source]
        source: crate::ContentDigestParseError,
    },
    /// Underlying byte reference is invalid.
    #[error(transparent)]
    InvalidContent(Box<ProjectError>),
    /// Table declaration is invalid.
    #[error(transparent)]
    InvalidTable(#[from] TableManifestError),
    /// Artifact record/locator/metadata is invalid.
    #[error(transparent)]
    InvalidRecord(#[from] ArtifactRecordError),
    /// Two serialized or batch records claim the same artifact ID.
    #[error("duplicate artifact ID {artifact}")]
    DuplicateArtifact {
        /// Duplicated ID.
        artifact: ArtifactId,
    },
    /// Artifact names itself as a dependency.
    #[error("artifact {artifact} depends on itself")]
    SelfDependency {
        /// Self-dependent artifact.
        artifact: ArtifactId,
    },
    /// Artifact dependency is absent.
    #[error("artifact {artifact} depends on missing artifact {dependency}")]
    MissingDependency {
        /// Dependent artifact.
        artifact: ArtifactId,
        /// Missing dependency.
        dependency: ArtifactId,
    },
    /// Dependency graph contains a cycle.
    #[error("artifact dependency cycle involves {artifacts:?}")]
    Cycle {
        /// Deterministically sorted nodes remaining after iterative sorting.
        artifacts: Vec<ArtifactId>,
    },
    /// Claimed semantic digest differs from the recomputed declaration.
    #[error(
        "artifact {artifact} semantic digest mismatch: expected {expected}, observed {observed}"
    )]
    SemanticDigestMismatch {
        /// Affected artifact.
        artifact: ArtifactId,
        /// Claimed digest.
        expected: ContentDigest,
        /// Recomputed digest.
        observed: ContentDigest,
    },
    /// Claimed artifact ID differs from recomputed schema/content identity.
    #[error("artifact ID mismatch: expected {expected}, observed {observed}")]
    ArtifactIdMismatch {
        /// Claimed ID.
        expected: ArtifactId,
        /// Recomputed ID.
        observed: ArtifactId,
    },
    /// Same artifact ID arrived with distinct non-location semantics.
    #[error("artifact {artifact} conflicts with its existing semantic declaration")]
    ConflictingArtifact {
        /// Conflicting ID.
        artifact: ArtifactId,
    },
    /// Metadata wire array repeats a key.
    #[error("duplicate semantic metadata key {key:?}")]
    DuplicateMetadataKey {
        /// Duplicated key.
        key: String,
    },
    /// Decoded bytes are valid but not the canonical fixed-point encoding.
    #[error("artifact catalog JSON is not canonical")]
    NonCanonicalCatalog,
}

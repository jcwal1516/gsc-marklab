use thiserror::Error;

/// Invalid embedding-domain value, encoding, dependency, or resource request.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EmbeddingError {
    /// A dimension or required pixel extent was zero.
    #[error("embedding dimension and required pixel extents must be positive")]
    ZeroDimension,
    /// A count, byte length, or index calculation overflowed.
    #[error("embedding size calculation overflowed")]
    SizeOverflow,
    /// A requested allocation exceeds its explicit retained-byte budget.
    #[error("embedding retained bytes {required} exceed budget {maximum}")]
    RetainedByteBudgetExceeded {
        /// Conservatively required retained bytes.
        required: usize,
        /// Caller-provided maximum retained bytes.
        maximum: usize,
    },
    /// An allocation failed after its size was checked.
    #[error("embedding allocation failed for {requested} bytes")]
    AllocationFailed {
        /// Requested allocation size.
        requested: usize,
    },
    /// Expected cells are not in strictly increasing canonical order.
    #[error("expected cells must be strictly increasing and unique")]
    NonCanonicalCellOrder,
    /// A selection-rule token violates the frozen grammar.
    #[error("expected-cell selection rule is invalid")]
    InvalidSelectionRule,
    /// A bounded binary encoding is malformed or noncanonical.
    #[error("embedding binary encoding is malformed or noncanonical")]
    InvalidBinaryEncoding,
    /// An encoded input exceeds its explicit byte budget.
    #[error("encoded input bytes {observed} exceed budget {maximum}")]
    EncodedByteBudgetExceeded {
        /// Observed input bytes.
        observed: usize,
        /// Caller-provided maximum input bytes.
        maximum: usize,
    },
    /// A source-local identifier violates its bounded privacy-safe grammar.
    #[error("source-local cell identifier is invalid")]
    InvalidSourceCellId,
    /// Identity-map source identifiers are not strictly increasing and unique.
    #[error("source-local identity-map keys must be strictly increasing and unique")]
    NonCanonicalSourceOrder,
    /// Identity-map canonical targets are duplicated or differ from the expected set.
    #[error("identity-map range must be a one-to-one match for the expected cell set")]
    IdentityMapRangeMismatch,
    /// Table rows do not exactly match the expected cells in canonical order.
    #[error("embedding rows must exactly match expected cells in canonical order")]
    RowSetMismatch,
    /// A row's vector presence is inconsistent with its extraction status.
    #[error("embedding row vector is inconsistent with its status")]
    StatusVectorMismatch,
    /// A present row has the wrong number of components.
    #[error("embedding row dimension mismatch: expected {expected}, observed {observed}")]
    DimensionMismatch {
        /// Declared table dimension.
        expected: usize,
        /// Observed row dimension.
        observed: usize,
    },
    /// A present vector component is NaN or infinite.
    #[error("embedding component at row {row}, column {column} is non-finite")]
    NonFiniteComponent {
        /// Zero-based canonical row index.
        row: usize,
        /// Zero-based component index.
        column: usize,
    },
    /// A row index is outside the table.
    #[error("embedding row index {index} is outside row count {row_count}")]
    RowOutOfBounds {
        /// Requested zero-based index.
        index: usize,
        /// Available row count.
        row_count: usize,
    },
    /// A positive rational is zero, unreduced, or has a zero denominator.
    #[error("positive rational must be nonzero, reduced, and have a nonzero denominator")]
    InvalidPositiveRational,
    /// Spatial-context frame, transform, scale, or patch semantics are inconsistent.
    #[error("embedding spatial context is inconsistent with the coordinate registry")]
    InvalidSpatialContext,
    /// Strict context JSON is malformed, unsupported, or not canonical.
    #[error("embedding spatial-context JSON is invalid or noncanonical")]
    InvalidCanonicalJson,
    /// Row-link rows do not exactly match the expected cells in canonical order.
    #[error("embedding row link must exactly match expected cells in canonical order")]
    RowLinkSetMismatch,
    /// An expected cell is not explicitly declared as a cell in the hierarchy.
    #[error("embedding row-link cell is absent from the hierarchy")]
    RowLinkCellMissingFromHierarchy,
    /// Source-cell row indices are not a unique gap-free zero-based domain.
    #[error("source-cell rows must be unique, gap-free, and zero-based")]
    InvalidSourceCellRows,
    /// Referenced source-embedding rows are not a unique gap-free zero-based domain.
    #[error("source-embedding rows must be unique, gap-free, and zero-based")]
    InvalidSourceEmbeddingRows,
    /// A row-link declaration exceeds the fixed artifact row-count limit.
    #[error("embedding row-link rows {observed} exceed maximum {maximum}")]
    RowCountExceeded {
        /// Observed canonical row count.
        observed: usize,
        /// Frozen maximum row count.
        maximum: usize,
    },
    /// A row-link status is inconsistent with source-embedding-row presence.
    #[error("embedding row-link status is inconsistent with source-embedding-row presence")]
    StatusEmbeddingRowMismatch,
    /// Artifact roles do not resolve to a set of distinct dependencies.
    #[error("embedding artifact dependencies must be distinct")]
    DuplicateArtifactDependency,
    /// A fixed-point decimal is malformed or noncanonical.
    #[error("embedding canonical decimal is invalid")]
    InvalidCanonicalDecimal,
    /// Provenance fields violate the frozen version-one contract.
    #[error("cell-embedding provenance is invalid or incomplete")]
    InvalidProvenance,
    /// One or more required provenance declarations are absent or unknown.
    #[error("cell-embedding provenance is incomplete")]
    ProvenanceIncomplete,
    /// Strict provenance JSON is malformed, unsupported, or not canonical.
    #[error("cell-embedding provenance JSON is invalid or noncanonical")]
    InvalidProvenanceJson,
}

use thiserror::Error;

/// Invalid multiscale embedding value, wire, hierarchy binding, or resource request.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MultiscaleEmbeddingError {
    /// A dimension or required spatial extent is zero.
    #[error("multiscale embedding dimensions and required extents must be positive")]
    ZeroDimension,
    /// A fixed dimension exceeds the version-one hard limit.
    #[error("multiscale embedding dimension {observed} exceeds maximum {maximum}")]
    DimensionExceeded {
        /// Observed dimension.
        observed: u32,
        /// Frozen version-one maximum.
        maximum: u32,
    },
    /// A row count exceeds the version-one hard limit.
    #[error("multiscale embedding rows {observed} exceed maximum {maximum}")]
    RowCountExceeded {
        /// Observed row count.
        observed: usize,
        /// Frozen version-one maximum.
        maximum: usize,
    },
    /// A count, byte length, coordinate endpoint, or index calculation overflowed.
    #[error("multiscale embedding size or coordinate calculation overflowed")]
    SizeOverflow,
    /// A retained allocation would exceed the caller's explicit budget.
    #[error("multiscale embedding retained bytes {required} exceed budget {maximum}")]
    RetainedByteBudgetExceeded {
        /// Conservatively required retained bytes.
        required: usize,
        /// Caller-provided maximum retained bytes.
        maximum: usize,
    },
    /// Peak construction storage would exceed the caller's explicit working budget.
    #[error("multiscale embedding working bytes {required} exceed budget {maximum}")]
    WorkingByteBudgetExceeded {
        /// Conservatively required peak construction bytes.
        required: usize,
        /// Caller-provided maximum working bytes.
        maximum: usize,
    },
    /// An allocation failed after its size was checked.
    #[error("multiscale embedding allocation failed for {requested} bytes")]
    AllocationFailed {
        /// Requested allocation size.
        requested: usize,
    },
    /// Encoded input exceeds the lower of caller and hard limits.
    #[error("multiscale encoded input bytes {observed} exceed budget {maximum}")]
    EncodedByteBudgetExceeded {
        /// Observed encoded byte count.
        observed: usize,
        /// Effective maximum encoded byte count.
        maximum: usize,
    },
    /// Decoded/transient semantic storage exceeds the caller's explicit budget.
    #[error("multiscale decoded bytes {required} exceed budget {maximum}")]
    DecodedByteBudgetExceeded {
        /// Conservatively required decoded bytes.
        required: usize,
        /// Caller-provided maximum decoded bytes.
        maximum: usize,
    },
    /// A canonical JSON document is malformed, unsupported, or not at its exact fixed point.
    #[error("multiscale embedding JSON is invalid or noncanonical")]
    InvalidCanonicalJson,
    /// A selection-rule token violates the frozen grammar.
    #[error("multiscale expected-set selection rule is invalid")]
    InvalidSelectionRule,
    /// Expected typed IDs are not strictly increasing and unique.
    #[error("multiscale expected IDs must be strictly increasing and unique")]
    NonCanonicalExpectedOrder,
    /// An expected set violates its entity-specific cardinality rule.
    #[error("multiscale expected-set cardinality is invalid")]
    InvalidExpectedSetCardinality,
    /// An expected entity or owning slide is absent or belongs to another slide.
    #[error("multiscale entity hierarchy ownership is invalid")]
    HierarchyOwnershipMismatch,
    /// Frame, transform, scale, receptive-field, patch-grid, or boundary semantics disagree.
    #[error("patch embedding context is inconsistent")]
    InvalidPatchContext,
    /// Footprint rows do not exactly match the expected patch set.
    #[error("patch footprints must exactly match expected patches in canonical order")]
    FootprintSetMismatch,
    /// One footprint violates checked half-open boundary semantics.
    #[error("patch footprint violates the declared boundary policy")]
    InvalidPatchFootprint,
    /// Expected/context/footprint bindings disagree during overlap derivation.
    #[error("patch overlap inputs do not share one exact expected set and context")]
    OverlapInputMismatch,
    /// Positive-area overlap edges exceed the version-one hard limit.
    #[error("patch overlap edges {observed} exceed maximum {maximum}")]
    OverlapEdgeCountExceeded {
        /// Observed edge count at the first rejected edge.
        observed: usize,
        /// Frozen version-one maximum.
        maximum: usize,
    },
    /// A cell anchor is NaN or infinite.
    #[error("cell-patch anchor coordinates must be finite")]
    InvalidCellPatchAnchor,
    /// Cell assignment rows do not exactly match the expected cell set.
    #[error("cell-patch assignments must exactly match expected cells in canonical order")]
    CellPatchSetMismatch,
    /// Cell anchors are declared in a frame other than the exact patch image frame.
    #[error("cell-patch anchor frame does not match the patch context")]
    CellPatchFrameMismatch,
    /// Expected cells are absent from the owning-slide hierarchy.
    #[error("cell-patch hierarchy ownership is invalid at assignment row {row}")]
    CellPatchHierarchyMismatch {
        /// Zero-based canonical assignment row.
        row: usize,
    },
    /// Expected-patch, context, footprint, or artifact bindings disagree.
    #[error("cell-patch inputs do not share one exact support binding")]
    CellPatchInputMismatch,
    /// Artifact roles alias where version one requires distinct dependencies.
    #[error("cell-patch artifact dependency roles must be distinct")]
    DuplicateCellPatchArtifactDependency,
    /// One declared interpolation contributor has a zero numerator or denominator.
    #[error("cell-patch contributor numerator and denominator must be positive")]
    InvalidCellPatchContributor,
    /// One interpolation group violates identity, order, count, or fraction rules.
    #[error("cell-patch contributors are invalid at assignment row {row}")]
    InvalidCellPatchContributors {
        /// Zero-based canonical assignment row.
        row: usize,
    },
    /// Cell-patch edges exceed the version-one hard limit.
    #[error("cell-patch edges {observed} exceed maximum {maximum}")]
    CellPatchEdgeCountExceeded {
        /// Observed edge count at the first rejected edge.
        observed: usize,
        /// Frozen version-one maximum.
        maximum: usize,
    },
    /// Indexed containment candidate checks exceed the caller's explicit work budget.
    #[error("cell-patch candidate checks {required} exceed budget {maximum}")]
    CellPatchCandidateCheckBudgetExceeded {
        /// Candidate count at the first rejected check.
        required: usize,
        /// Caller-provided maximum checks per deterministic pass.
        maximum: usize,
    },
    /// A declared patch-region fraction violates its relation-specific canonical rule.
    #[error("patch-region overlap fraction is invalid")]
    InvalidPatchRegionFraction,
    /// Expected/context/footprint bindings disagree for a patch-region assessment.
    #[error("patch-region inputs do not share one exact support binding")]
    PatchRegionInputMismatch,
    /// Patch-region artifact roles alias where version one requires distinct dependencies.
    #[error("patch-region artifact dependency roles must be distinct")]
    DuplicatePatchRegionArtifactDependency,
    /// Nonzero declarations are unordered, duplicated, unknown, or relation-invalid.
    #[error("patch-region declarations are invalid at nonzero row {row}")]
    InvalidPatchRegionDeclarations {
        /// Zero-based supplied nonzero row.
        row: usize,
    },
    /// The exhaustive expected-patch by expected-region product overflowed.
    #[error("patch-region assessed Cartesian pair count overflowed")]
    PatchRegionPairCountOverflow,
    /// Nonzero patch-region declarations exceed the version-one hard limit.
    #[error("patch-region nonzero rows {observed} exceed maximum {maximum}")]
    PatchRegionRowCountExceeded {
        /// Observed nonzero row count.
        observed: usize,
        /// Frozen version-one maximum.
        maximum: usize,
    },
    /// A source-local patch key is empty, oversized, padded, or control-bearing.
    #[error("patch source identity is invalid")]
    InvalidPatchSourceKey,
    /// A patch source-profile token violates the frozen grammar.
    #[error("patch source profile is invalid")]
    InvalidPatchSourceProfile,
    /// Source keys, source rows, or source-set cardinality are noncanonical.
    #[error("patch source entities are noncanonical")]
    NonCanonicalPatchSourceEntities,
    /// A source-domain, expected-range, artifact, or logical identity binding disagrees.
    #[error("patch identity map does not match its exact source and expected sets")]
    PatchIdentityMapMismatch,
    /// Source-row entries do not match the exact source, identity-map, and expected domains.
    #[error("patch source-row link is inconsistent")]
    PatchSourceRowLinkMismatch,
    /// Embedding status and nullable source-vector row disagree.
    #[error("patch source-row status and vector-row presence disagree")]
    PatchSourceRowStatusMismatch,
    /// Artifact roles alias where the patch source chain requires distinct dependencies.
    #[error("patch source artifact dependency roles must be distinct")]
    DuplicatePatchSourceArtifactDependency,
    /// A patch-normalization decimal violates its exact bounded fixed-point grammar.
    #[error("patch input normalization decimal is invalid")]
    InvalidPatchNormalizationDecimal,
    /// Patch input normalization differs from the closed RGB H&E version-one profile.
    #[error("patch input normalization is invalid")]
    InvalidPatchInputNormalization,
    /// Table rows do not exactly match the expected typed set.
    #[error("multiscale embedding rows must exactly match the expected set")]
    RowSetMismatch,
    /// A row's vector presence disagrees with its extraction status.
    #[error("multiscale embedding row vector is inconsistent with its status")]
    StatusVectorMismatch,
    /// A present row has the wrong component count.
    #[error("multiscale embedding dimension mismatch: expected {expected}, observed {observed}")]
    DimensionMismatch {
        /// Declared component count.
        expected: usize,
        /// Observed component count.
        observed: usize,
    },
    /// A present vector component is NaN or infinite.
    #[error("multiscale embedding component at row {row}, column {column} is non-finite")]
    NonFiniteComponent {
        /// Zero-based canonical row index.
        row: usize,
        /// Zero-based component index.
        column: usize,
    },
    /// A row index is outside the table.
    #[error("multiscale embedding row index {index} is outside row count {row_count}")]
    RowOutOfBounds {
        /// Requested zero-based row.
        index: usize,
        /// Available canonical row count.
        row_count: usize,
    },
    /// A requested borrowed row block is outside the table or overflows.
    #[error(
        "multiscale row block start {start} with count {row_count} is outside table rows {table_row_count}"
    )]
    RowBlockOutOfBounds {
        /// Requested zero-based first row.
        start: usize,
        /// Requested number of rows.
        row_count: usize,
        /// Available canonical row count.
        table_row_count: usize,
    },
    /// A bounded QC scan requires a positive maximum block size.
    #[error("multiscale QC scan maximum block rows must be positive")]
    ZeroScanBlockRows,
}

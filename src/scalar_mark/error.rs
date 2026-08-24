use marklab_workflow::ArtifactId;
use thiserror::Error;

use crate::errors::MarklabError;

/// Failure to declare, bind, or execute the version-one scalar marked-pattern input.
#[derive(Debug, Error)]
pub enum DeclaredScalarInputError {
    /// Stable scalar-mark ID violates the bounded token grammar.
    #[error("scalar mark ID must be a 1-128 byte ASCII token")]
    InvalidMarkId,
    /// Binary display label is empty, untrimmed, too long, or contains control characters.
    #[error("binary mark label must be 1-256 trimmed non-control UTF-8 bytes")]
    InvalidMarkLabel,
    /// A row-level scalar mark was incorrectly declared as a derived aggregate.
    #[error("derived_summary is not a supported per-cell scalar measurement status")]
    UnsupportedPerCellMeasurementStatus,
    /// Probability threshold is non-finite or outside the closed unit interval.
    #[error("probability threshold must be finite and in [0, 1]")]
    InvalidThreshold,
    /// A thresholded binary declaration omitted its required evidence artifact.
    #[error("thresholded binary mark requires threshold provenance")]
    ThresholdProvenanceMissing,
    /// Pattern and declared CellId rows have different lengths.
    #[error("declared scalar input has {observed} CellIds; expected {expected}")]
    CellIdCountMismatch {
        /// Pattern row count.
        expected: usize,
        /// Supplied CellId count.
        observed: usize,
    },
    /// Caller supplied a zero construction resource limit.
    #[error("declared scalar input resource limits must be positive")]
    InvalidResourceLimit,
    /// Row count exceeds the caller's retained-input bound.
    #[error("declared scalar input has {observed} rows; maximum is {maximum}")]
    RowCountExceeded {
        /// Supplied row count.
        observed: usize,
        /// Caller maximum.
        maximum: usize,
    },
    /// CellId text exceeds the caller's retained-input bound.
    #[error("declared CellId text requires {required} bytes; maximum is {maximum}")]
    CellIdTextBudgetExceeded {
        /// Checked sum of CellId UTF-8 bytes.
        required: usize,
        /// Caller maximum.
        maximum: usize,
    },
    /// CellIds are duplicated or not in strictly increasing order.
    #[error("declared CellIds are not canonical at row {row}")]
    NonCanonicalCellIds {
        /// First invalid row.
        row: usize,
    },
    /// Project has no installed hierarchy.
    #[error("declared scalar input requires an installed cohort hierarchy")]
    CohortHierarchyMissing,
    /// One CellId is absent from, or not contained by the declared slide in, the hierarchy.
    #[error("declared CellId at row {row} is not owned by the declared slide")]
    CellHierarchyMismatch {
        /// Invalid row.
        row: usize,
    },
    /// The explicit slide is absent from the project hierarchy or conflicts with compatibility metadata.
    #[error("declared owning slide does not match the project hierarchy or Pattern metadata")]
    OwningSlideMismatch,
    /// Project has no installed coordinate registry.
    #[error("declared scalar input requires an installed coordinate registry")]
    CoordinateRegistryMissing,
    /// Frame is absent or is not exact physical 2-D micrometre `[X,Y]`.
    #[error("declared coordinate frame must be installed physical 2-D micrometre [X,Y]")]
    CoordinateFrameMismatch,
    /// A required or retained optional Pattern column has a different row count.
    #[error("Pattern column {column} has {observed} rows; expected {expected}")]
    PatternColumnLengthMismatch {
        /// Stable compatibility column name.
        column: &'static str,
        /// Pattern row count.
        expected: usize,
        /// Observed column length.
        observed: usize,
    },
    /// Compatibility row validity marks a row unavailable, which version one cannot represent.
    #[error("Pattern row {row} is not present; scalar missing rows are unsupported")]
    UnsupportedMissingScalarRow {
        /// First unavailable row.
        row: usize,
    },
    /// A compatibility coordinate is non-finite.
    #[error("Pattern coordinate at row {row} is non-finite")]
    NonFiniteCoordinate {
        /// Invalid row.
        row: usize,
    },
    /// A compatibility binary value is not zero or one.
    #[error("Pattern binary mark at row {row} is not 0 or 1")]
    InvalidBinaryMark {
        /// Invalid row.
        row: usize,
    },
    /// Probability values and their declaration are not both absent or both present.
    #[error("probability values and declaration must be present together")]
    ProbabilityDeclarationMismatch,
    /// Dense probability column has the wrong number of rows.
    #[error("probability column has {observed} rows; expected {expected}")]
    ProbabilityLengthMismatch {
        /// Pattern row count.
        expected: usize,
        /// Probability row count.
        observed: usize,
    },
    /// A probability value is non-finite or outside the closed unit interval.
    #[error("probability at row {row} must be finite and in [0, 1]")]
    InvalidProbability {
        /// Invalid row.
        row: usize,
    },
    /// One declared semantic artifact is reused for another role.
    #[error("declared scalar semantic artifact roles must be distinct")]
    DuplicateArtifactRole,
    /// Required provenance record is absent from the project catalog.
    #[error("declared provenance record {artifact} is missing")]
    ProvenanceRecordMissing {
        /// Missing artifact.
        artifact: ArtifactId,
    },
    /// Provenance schema, metadata, table shape, or dependencies differ from the exact profile.
    #[error("declared provenance record {artifact} does not match its exact profile")]
    ProvenanceProfileMismatch {
        /// Mismatched artifact.
        artifact: ArtifactId,
    },
    /// Threshold source mark ID or measurement status disagrees with the probability declaration.
    #[error("threshold source does not match the declared probability mark")]
    ThresholdSourceMismatch,
    /// Binary row disagrees with its exact probability comparator and threshold.
    #[error("thresholded binary mark disagrees with its probability at row {row}")]
    ThresholdBindingMismatch {
        /// First mismatched row.
        row: usize,
    },
    /// Config 0.2 mark label differs from the declared binary label.
    #[error("analysis.mark_label does not match the declared binary mark label")]
    ConfigurationMarkLabelMismatch,
    /// Config 0.2 probability mode disagrees with declared value kinds/origin.
    #[error("analysis.use_probabilistic_marks does not match the declared scalar input")]
    ConfigurationValueKindMismatch,
    /// Deterministic declared-input identity could not be produced.
    #[error("declared scalar input identity overflow")]
    LogicalIdentityOverflow,
    /// Borrowed declared-input identity no longer matches its construction-time reference.
    #[error("declared scalar input identity changed after construction")]
    DeclaredIdentityMismatch,
    /// Unchanged compatibility analysis failed.
    #[error("declared scalar analysis failed: {source}")]
    Analysis {
        /// Existing compatibility-engine error.
        #[source]
        source: MarklabError,
    },
}

impl DeclaredScalarInputError {
    /// Wrap an error returned by the unchanged marked-analysis engine.
    pub(crate) fn analysis(source: MarklabError) -> Self {
        Self::Analysis { source }
    }
}

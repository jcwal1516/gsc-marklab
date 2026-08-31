use marklab_cohort::InferenceAlternative;
use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::ContentDigest;
use thiserror::Error;

use crate::scalar_mark::ScalarMarkId;

/// Explicit adjacency normalization for global Moran's I.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalMoranWeightPolicy {
    /// Every directed radius-neighbour edge has weight one.
    BinarySymmetric,
    /// Each outgoing radius-neighbour row sums to one.
    RowStandardized,
}

impl GlobalMoranWeightPolicy {
    pub(super) fn wire_name(self) -> &'static str {
        match self {
            Self::BinarySymmetric => "binary_symmetric",
            Self::RowStandardized => "row_standardized",
        }
    }
}

/// Prespecified tail for the global Moran random-labeling test.
pub type GlobalMoranAlternative = InferenceAlternative;

/// Conditioning used by the admitted method-specific random-labeling design.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalMoranConditioning {
    /// Permute complete scalar values across all rows.
    None,
    /// Permute only within the exact `histologic_compartment` row codes.
    HistologicCompartment,
}

/// Explicit random-labeling controls accepted by global Moran's I.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalMoranDesign {
    pub(super) conditioning: GlobalMoranConditioning,
    pub(super) permutations: usize,
    pub(super) seed: u64,
    pub(super) alternative: GlobalMoranAlternative,
}

impl GlobalMoranDesign {
    /// Declare unstratified whole-value random labeling.
    pub fn random_labeling(
        permutations: usize,
        seed: u64,
        alternative: GlobalMoranAlternative,
    ) -> Result<Self, GlobalMoranError> {
        Self::new(
            GlobalMoranConditioning::None,
            permutations,
            seed,
            alternative,
        )
    }

    /// Declare whole-value random labeling within exact histologic compartments.
    pub fn histologic_compartment_random_labeling(
        permutations: usize,
        seed: u64,
        alternative: GlobalMoranAlternative,
    ) -> Result<Self, GlobalMoranError> {
        Self::new(
            GlobalMoranConditioning::HistologicCompartment,
            permutations,
            seed,
            alternative,
        )
    }

    fn new(
        conditioning: GlobalMoranConditioning,
        permutations: usize,
        seed: u64,
        alternative: GlobalMoranAlternative,
    ) -> Result<Self, GlobalMoranError> {
        if permutations == 0 {
            return Err(GlobalMoranError::InvalidDesign(
                "global Moran inference requires at least one permutation".into(),
            ));
        }
        Ok(Self {
            conditioning,
            permutations,
            seed,
            alternative,
        })
    }

    /// Declared conditioning policy.
    pub fn conditioning(&self) -> GlobalMoranConditioning {
        self.conditioning
    }

    /// Requested deterministic permutation count.
    pub fn permutations(&self) -> usize {
        self.permutations
    }

    /// Base seed before method-domain separation.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Prespecified alternative.
    pub fn alternative(&self) -> GlobalMoranAlternative {
        self.alternative
    }
}

/// Explicit resource ceilings for one global Moran workflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlobalMoranLimits {
    /// Maximum point rows.
    pub maximum_points: usize,
    /// Maximum directed neighbour edges retained once.
    pub maximum_directed_edges: usize,
    /// Maximum directed-edge evaluations across all permutations.
    pub maximum_permutation_edge_evaluations: usize,
}

impl GlobalMoranLimits {
    /// Validate positive point, edge, and permutation-work limits.
    pub fn new(
        maximum_points: usize,
        maximum_directed_edges: usize,
        maximum_permutation_edge_evaluations: usize,
    ) -> Result<Self, GlobalMoranError> {
        if maximum_points == 0
            || maximum_directed_edges == 0
            || maximum_permutation_edge_evaluations == 0
        {
            return Err(GlobalMoranError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_directed_edges,
            maximum_permutation_edge_evaluations,
        })
    }
}

/// Complete typed result of one global Moran random-labeling workflow.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalMoranResult {
    /// Stable continuous mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measured or predicted status.
    pub measurement_status: MeasurementStatus,
    /// Exact physical coordinate frame shared by points and window.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Fixed neighbour radius in micrometres.
    pub radius_um: f64,
    /// Explicit adjacency normalization.
    pub weight_policy: GlobalMoranWeightPolicy,
    /// Content identity of the exact ordered directed edge plan and policy.
    pub weights_digest: ContentDigest,
    /// Number of point rows.
    pub point_count: usize,
    /// Number of retained directed edges.
    pub directed_edge_count: usize,
    /// Number of exchangeability strata.
    pub stratum_count: usize,
    /// Typed categorical mark used for conditioning, absent when unstratified.
    pub conditioning_mark_id: Option<ScalarMarkId>,
    /// Measurement status of the conditioning mark, absent when unstratified.
    pub conditioning_measurement_status: Option<MeasurementStatus>,
    /// Observed global Moran's I.
    pub statistic: f64,
    /// Random-labeling expectation `-1/(n-1)`.
    pub null_expectation: f64,
    /// Inclusive-plus-one permutation p-value.
    pub p_value: f64,
    /// Requested permutations.
    pub permutations_requested: usize,
    /// Attempted permutations.
    pub permutations_attempted: usize,
    /// Successfully completed permutations.
    pub permutations_completed: usize,
    /// Exact base seed.
    pub seed: u64,
    /// Prespecified alternative.
    pub alternative: GlobalMoranAlternative,
    /// Explicit random-labeling conditioning.
    pub conditioning: GlobalMoranConditioning,
}

/// Prespecified tail for the global Geary random-labeling test.
pub type GlobalGearyAlternative = GlobalMoranAlternative;

/// Explicit random-labeling controls accepted by global Geary's C.
pub type GlobalGearyDesign = GlobalMoranDesign;

/// Explicit resource ceilings for one global Geary workflow.
pub type GlobalGearyLimits = GlobalMoranLimits;

/// Complete typed result of one global Geary random-labeling workflow.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalGearyResult {
    /// Stable continuous mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measured or predicted status.
    pub measurement_status: MeasurementStatus,
    /// Exact physical coordinate frame shared by points and window.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Fixed neighbour radius in micrometres.
    pub radius_um: f64,
    /// Explicit adjacency normalization.
    pub weight_policy: GlobalMoranWeightPolicy,
    /// Content identity of the exact ordered directed edge plan and policy.
    pub weights_digest: ContentDigest,
    /// Number of point rows.
    pub point_count: usize,
    /// Number of retained directed edges.
    pub directed_edge_count: usize,
    /// Number of exchangeability strata.
    pub stratum_count: usize,
    /// Typed categorical mark used for conditioning, absent when unstratified.
    pub conditioning_mark_id: Option<ScalarMarkId>,
    /// Measurement status of the conditioning mark, absent when unstratified.
    pub conditioning_measurement_status: Option<MeasurementStatus>,
    /// Observed global Geary's C.
    pub statistic: f64,
    /// Random-labeling expectation, exactly one.
    pub null_expectation: f64,
    /// Inclusive-plus-one permutation p-value.
    pub p_value: f64,
    /// Requested permutations.
    pub permutations_requested: usize,
    /// Attempted permutations.
    pub permutations_attempted: usize,
    /// Successfully completed permutations.
    pub permutations_completed: usize,
    /// Exact base seed.
    pub seed: u64,
    /// Prespecified alternative.
    pub alternative: GlobalGearyAlternative,
    /// Explicit random-labeling conditioning.
    pub conditioning: GlobalMoranConditioning,
}

/// Invalid typed input, design, resource, geometry, or numerical state for global Moran's I.
pub type GlobalMoranError = GlobalSpatialAutocorrelationError;

/// Invalid typed input, design, resource, geometry, or numerical state for global Geary's C.
pub type GlobalGearyError = GlobalSpatialAutocorrelationError;

/// Shared failure states of the admitted global spatial-autocorrelation workflows.
#[derive(Debug, Error, PartialEq)]
pub enum GlobalSpatialAutocorrelationError {
    /// One or more resource ceilings are zero.
    #[error("global spatial-autocorrelation resource limits must be positive")]
    InvalidResourceLimit,
    /// Method-specific inference controls are invalid.
    #[error("invalid global spatial-autocorrelation design: {0}")]
    InvalidDesign(String),
    /// Neighbour radius is non-finite or non-positive.
    #[error("global spatial-autocorrelation neighbour radius must be finite and positive")]
    InvalidRadius,
    /// Exact observation window has no installed frame binding.
    #[error("global spatial autocorrelation requires a coordinate-frame-bound observation window")]
    UnboundObservationWindow,
    /// Point and window frames differ.
    #[error(
        "global spatial-autocorrelation frame mismatch: expected {expected}, observed {observed}"
    )]
    CoordinateFrameMismatch {
        /// Point/mark input frame.
        expected: CoordinateFrameId,
        /// Window frame.
        observed: CoordinateFrameId,
    },
    /// Fewer than three point rows were supplied.
    #[error("global spatial autocorrelation requires at least three points; observed {observed}")]
    InsufficientPoints {
        /// Observed row count.
        observed: usize,
    },
    /// Point rows exceed the explicit limit.
    #[error("global spatial autocorrelation has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed rows.
        observed: usize,
        /// Explicit maximum.
        maximum: usize,
    },
    /// A point is outside the exact observation window.
    #[error("global spatial-autocorrelation point row {row} lies outside the observation window")]
    PointOutsideWindow {
        /// Outside row.
        row: usize,
    },
    /// Two rows have the same physical coordinate and no duplicate policy was declared.
    #[error("global spatial-autocorrelation duplicate point rows {first_row} and {second_row}")]
    DuplicatePoint {
        /// First duplicate row.
        first_row: usize,
        /// Second duplicate row.
        second_row: usize,
    },
    /// Input was not constructed from the typed mark table adapter.
    #[error("global spatial autocorrelation requires a typed MarkTable input")]
    TypedMarkTableRequired,
    /// Requested continuous mark is absent.
    #[error("global spatial-autocorrelation continuous mark {mark_id:?} is absent")]
    ContinuousMarkMissing {
        /// Missing mark identity.
        mark_id: ScalarMarkId,
    },
    /// An aligned row vector has the wrong length.
    #[error("global spatial-autocorrelation row count mismatch: expected {expected}, observed {observed}")]
    RowCountMismatch {
        /// Required rows.
        expected: usize,
        /// Observed rows.
        observed: usize,
    },
    /// Spatial index or query construction failed.
    #[error("global spatial-autocorrelation geometry failed: {0}")]
    Geometry(String),
    /// At least one point has no neighbour under the declared radius.
    #[error(
        "global spatial-autocorrelation point row {row} is isolated under the declared weights"
    )]
    IsolatedPoint {
        /// Isolated row.
        row: usize,
    },
    /// Directed edge count exceeds the explicit limit.
    #[error("global spatial-autocorrelation directed-edge count exceeds {maximum}")]
    DirectedEdgeLimitExceeded {
        /// Explicit maximum.
        maximum: usize,
    },
    /// Permutation-by-edge work exceeds the explicit limit.
    #[error("global spatial-autocorrelation permutation work is {observed}; maximum is {maximum}")]
    PermutationWorkExceeded {
        /// Required directed-edge evaluations.
        observed: usize,
        /// Explicit maximum.
        maximum: usize,
    },
    /// Histologic-compartment conditioning was requested but not supplied.
    #[error("global spatial-autocorrelation histologic_compartment stratum is missing")]
    MissingCompartmentStratum,
    /// No declared stratum contains exchangeable distinct values.
    #[error("global spatial-autocorrelation random-labeling null is degenerate")]
    DegenerateNull,
    /// Continuous values have zero variance.
    #[error("global spatial-autocorrelation continuous mark has zero variance")]
    ZeroVariance,
    /// Checked count or work arithmetic overflowed.
    #[error("global spatial-autocorrelation size arithmetic overflow")]
    SizeOverflow,
    /// Finite inputs produced a non-finite statistic or p-value.
    #[error("global spatial autocorrelation produced a non-finite numerical result")]
    NumericalFailure,
}

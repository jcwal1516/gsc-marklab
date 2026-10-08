use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::ContentDigest;
use thiserror::Error;

use crate::scalar_mark::ScalarMarkId;

mod evaluation;
mod validation;
mod workflow;

use validation::normalize_zero;
pub(crate) use validation::pair_plan_digest;
pub use workflow::{scalar_semivariogram, scalar_semivariogram_permutation};

/// One half-open physical lag interval `[lower_um, upper_um)`, except the final bin includes its
/// upper edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalarVariogramBin {
    lower_um: f64,
    upper_um: f64,
}

impl ScalarVariogramBin {
    /// Validate one finite nonnegative increasing lag interval.
    pub fn new(lower_um: f64, upper_um: f64) -> Result<Self, ScalarVariogramError> {
        if !lower_um.is_finite() || !upper_um.is_finite() || lower_um < 0.0 || upper_um <= lower_um
        {
            return Err(ScalarVariogramError::InvalidBins);
        }
        Ok(Self {
            lower_um: normalize_zero(lower_um),
            upper_um: normalize_zero(upper_um),
        })
    }

    /// Inclusive lower lag in micrometres.
    pub fn lower_um(self) -> f64 {
        self.lower_um
    }

    /// Upper lag in micrometres, inclusive only for the final declared bin.
    pub fn upper_um(self) -> f64 {
        self.upper_um
    }
}

/// Explicit point and unordered-pair ceilings for one observed scalar semivariogram.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarVariogramLimits {
    /// Maximum point rows.
    pub maximum_points: usize,
    /// Maximum unordered pair visits, including pairs outside the declared lag range.
    pub maximum_pair_visits: usize,
}

impl ScalarVariogramLimits {
    /// Validate positive point and pair-visit ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_pair_visits: usize,
    ) -> Result<Self, ScalarVariogramError> {
        if maximum_points == 0 || maximum_pair_visits == 0 {
            return Err(ScalarVariogramError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_pair_visits,
        })
    }
}

/// Whole-value random-labeling controls for one scalar-semivariogram global envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramInferenceDesign {
    conditioning: ScalarVariogramConditioning,
    permutations: usize,
    seed: u64,
    alpha: f64,
}

impl ScalarVariogramInferenceDesign {
    /// Declare unstratified whole-value random labeling across fixed cell locations.
    pub fn random_labeling(
        permutations: usize,
        seed: u64,
        alpha: f64,
    ) -> Result<Self, ScalarVariogramError> {
        Self::new(ScalarVariogramConditioning::None, permutations, seed, alpha)
    }

    /// Declare whole-value random labeling within exact typed histologic compartments.
    pub fn histologic_compartment_random_labeling(
        permutations: usize,
        seed: u64,
        alpha: f64,
    ) -> Result<Self, ScalarVariogramError> {
        Self::new(
            ScalarVariogramConditioning::HistologicCompartment,
            permutations,
            seed,
            alpha,
        )
    }

    fn new(
        conditioning: ScalarVariogramConditioning,
        permutations: usize,
        seed: u64,
        alpha: f64,
    ) -> Result<Self, ScalarVariogramError> {
        let curve_count = permutations
            .checked_add(1)
            .ok_or(ScalarVariogramError::SizeOverflow)?;
        if permutations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || curve_count as f64 * alpha < 1.0
        {
            return Err(ScalarVariogramError::InvalidInferenceDesign);
        }
        Ok(Self {
            conditioning,
            permutations,
            seed,
            alpha,
        })
    }

    /// Declared random-labeling conditioning.
    pub fn conditioning(&self) -> ScalarVariogramConditioning {
        self.conditioning
    }

    /// Requested deterministic permutation count.
    pub fn permutations(&self) -> usize {
        self.permutations
    }

    /// Base deterministic seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Family-wise alpha for the two-sided ERL global envelope.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
}

/// Conditioning admitted for scalar-semivariogram random labeling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarVariogramConditioning {
    /// Permute complete scalar values across every row.
    None,
    /// Permute complete scalar values only within exact histologic-compartment codes.
    HistologicCompartment,
}

/// Resource ceilings for observed and permuted scalar-semivariogram evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarVariogramInferenceLimits {
    observed: ScalarVariogramLimits,
    maximum_permutation_pair_evaluations: usize,
}

impl ScalarVariogramInferenceLimits {
    /// Validate point, all-pair, and permutation-by-pair ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_pair_visits: usize,
        maximum_permutation_pair_evaluations: usize,
    ) -> Result<Self, ScalarVariogramError> {
        if maximum_permutation_pair_evaluations == 0 {
            return Err(ScalarVariogramError::InvalidResourceLimit);
        }
        Ok(Self {
            observed: ScalarVariogramLimits::new(maximum_points, maximum_pair_visits)?,
            maximum_permutation_pair_evaluations,
        })
    }

    pub(crate) fn observed(self) -> ScalarVariogramLimits {
        self.observed
    }

    pub(crate) fn maximum_permutation_pair_evaluations(self) -> usize {
        self.maximum_permutation_pair_evaluations
    }
}

/// One observed scalar semivariance lag row.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramRow {
    /// Inclusive lower lag in micrometres.
    pub lower_um: f64,
    /// Upper lag in micrometres.
    pub upper_um: f64,
    /// Whether this row includes its upper edge; true only for the final row.
    pub upper_inclusive: bool,
    /// Exact unordered pair count in this bin.
    pub pair_count: usize,
    /// Half the mean squared scalar difference, absent for an empty bin.
    pub semivariance: Option<f64>,
}

/// Complete bounded observed scalar semivariogram.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramResult {
    /// Stable continuous mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measured or predicted status.
    pub measurement_status: MeasurementStatus,
    /// Exact physical coordinate frame shared by points and window.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Exact typed input identity.
    pub declared_input_digest: ContentDigest,
    /// Canonical row, coordinate, window, mark, and bin identity.
    pub pair_plan_digest: ContentDigest,
    /// Number of point rows.
    pub point_count: usize,
    /// Exact unordered pairs visited.
    pub pair_visits: usize,
    /// Observed lag curve.
    pub curve: Vec<ScalarVariogramRow>,
    /// Explicit edge-correction status.
    pub edge_correction: &'static str,
    /// Explicit inference status.
    pub inference_status: &'static str,
}

/// One observed semivariance row with its simultaneous global envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramEnvelopeRow {
    /// Exact observed lag row.
    pub observed: ScalarVariogramRow,
    /// Lower simultaneous ERL bound.
    pub lower_global_envelope: Option<f64>,
    /// Upper simultaneous ERL bound.
    pub upper_global_envelope: Option<f64>,
}

/// Complete blocked-permutation scalar-semivariogram global inference result.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarVariogramInferenceResult {
    /// Unchanged observed result.
    pub observed: ScalarVariogramResult,
    /// Observed rows with simultaneous bounds.
    pub curve: Vec<ScalarVariogramEnvelopeRow>,
    /// Inclusive-plus-one global p-value.
    pub p_global: f64,
    /// Observed ERL depth.
    pub observed_erl_depth: f64,
    /// Critical retained-curve depth.
    pub critical_erl_depth: f64,
    /// Family-wise alpha.
    pub alpha: f64,
    /// Number of nonempty bins.
    pub eligible_bin_count: usize,
    /// Number of exchangeability strata.
    pub stratum_count: usize,
    /// Optional conditioning mark.
    pub conditioning_mark_id: Option<ScalarMarkId>,
    /// Optional conditioning measurement status.
    pub conditioning_measurement_status: Option<MeasurementStatus>,
    /// Random-labeling conditioning.
    pub conditioning: ScalarVariogramConditioning,
    /// Requested permutations.
    pub permutations_requested: usize,
    /// Completed permutations.
    pub permutations_completed: usize,
    /// Deterministic seed.
    pub seed: u64,
    /// Curve-family multiplicity policy.
    pub multiplicity_policy: &'static str,
}

/// Invalid scalar-semivariogram input, plan, resource, or numerical state.
#[derive(Debug, Error, PartialEq)]
pub enum ScalarVariogramError {
    /// Typed input identity could not be recomputed.
    #[error("scalar variogram input identity is invalid: {0}")]
    InvalidInput(String),
    /// Lag bins are invalid.
    #[error("scalar variogram bins must be finite, increasing, nonnegative, and contiguous")]
    InvalidBins,
    /// A resource ceiling is zero.
    #[error("scalar variogram resource limits must be positive")]
    InvalidResourceLimit,
    /// Permutation count or alpha is invalid.
    #[error("scalar variogram inference requires positive permutations, alpha in (0,1), and (B+1)*alpha >= 1")]
    InvalidInferenceDesign,
    /// Observation window lacks a frame.
    #[error("scalar variogram requires a coordinate-frame-bound observation window")]
    UnboundObservationWindow,
    /// Point and window frames differ.
    #[error("scalar variogram frame mismatch: expected {expected}, observed {observed}")]
    CoordinateFrameMismatch {
        /// Input frame.
        expected: CoordinateFrameId,
        /// Window frame.
        observed: CoordinateFrameId,
    },
    /// Fewer than two points were supplied.
    #[error("scalar variogram requires at least two points")]
    InsufficientPoints,
    /// Point rows exceed the explicit limit.
    #[error("scalar variogram has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed rows.
        observed: usize,
        /// Maximum rows.
        maximum: usize,
    },
    /// Pair visits exceed the explicit limit.
    #[error("scalar variogram pair visits are {observed}; maximum is {maximum}")]
    PairVisitLimitExceeded {
        /// Observed visits.
        observed: usize,
        /// Maximum visits.
        maximum: usize,
    },
    /// Permutation work exceeds the ceiling.
    #[error("scalar variogram permutation work is {observed}; maximum is {maximum}")]
    PermutationWorkExceeded {
        /// Required work.
        observed: usize,
        /// Maximum work.
        maximum: usize,
    },
    /// Coordinate columns have different lengths.
    #[error("scalar variogram coordinate row mismatch: x has {x_rows}, y has {y_rows}")]
    PointShapeMismatch {
        /// X rows.
        x_rows: usize,
        /// Y rows.
        y_rows: usize,
    },
    /// A point lies outside the window.
    #[error("scalar variogram point row {row} lies outside the observation window")]
    PointOutsideWindow {
        /// Outside row.
        row: usize,
    },
    /// Two point rows share coordinates.
    #[error("scalar variogram duplicate point rows {first_row} and {second_row}")]
    DuplicatePoint {
        /// First row.
        first_row: usize,
        /// Second row.
        second_row: usize,
    },
    /// Typed mark table is absent.
    #[error("scalar variogram requires a typed MarkTable input")]
    TypedMarkTableRequired,
    /// Continuous mark is absent.
    #[error("scalar variogram continuous mark {mark_id:?} is absent")]
    ContinuousMarkMissing {
        /// Missing mark.
        mark_id: ScalarMarkId,
    },
    /// Mark rows do not match point rows.
    #[error("scalar variogram row count mismatch: expected {expected}, observed {observed}")]
    RowCountMismatch {
        /// Expected rows.
        expected: usize,
        /// Observed rows.
        observed: usize,
    },
    /// Conditioning stratum is absent.
    #[error("scalar variogram histologic_compartment stratum is missing")]
    MissingCompartmentStratum,
    /// Null has no exchangeable distinct values.
    #[error("scalar variogram random-labeling null is degenerate")]
    DegenerateNull,
    /// Every lag bin is empty.
    #[error("scalar variogram has no nonempty bin eligible for curve inference")]
    NoEligibleBins,
    /// Shared inference failed.
    #[error("scalar variogram inference failed: {0}")]
    Inference(String),
    /// Count arithmetic overflowed.
    #[error("scalar variogram size arithmetic overflow")]
    SizeOverflow,
    /// Finite inputs produced a non-finite value.
    #[error("scalar variogram produced a non-finite numerical result")]
    NumericalFailure,
}

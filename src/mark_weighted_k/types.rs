use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::ContentDigest;
use thiserror::Error;

use crate::{ClassicalWindowSummary, ScalarMarkId};

/// Exact resource ceilings for one continuous mark-weighted K workflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkWeightedKLimits {
    /// Maximum retained point rows.
    pub maximum_points: usize,
    /// Maximum cumulative radii.
    pub maximum_radii: usize,
    /// Maximum directed pairs retained once.
    pub maximum_directed_pairs: usize,
    /// Maximum null directed-pair evaluations.
    pub maximum_permutation_pair_evaluations: usize,
    /// Maximum conservatively estimated retained bytes.
    pub maximum_retained_bytes: usize,
}

impl MarkWeightedKLimits {
    /// Validate positive point, radius, pair, null-work, and memory ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_directed_pairs: usize,
        maximum_permutation_pair_evaluations: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MarkWeightedKError> {
        if [
            maximum_points,
            maximum_radii,
            maximum_directed_pairs,
            maximum_permutation_pair_evaluations,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(MarkWeightedKError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_radii,
            maximum_directed_pairs,
            maximum_permutation_pair_evaluations,
            maximum_retained_bytes,
        })
    }
}

/// Frozen positive mark, radius axis, random-label controls, and resource policy.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkWeightedKConfig {
    pub(super) mark_id: ScalarMarkId,
    pub(super) radii_um: Box<[f64]>,
    pub(super) permutations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: MarkWeightedKLimits,
}

impl MarkWeightedKConfig {
    /// Validate one cumulative product-weighted K design.
    pub fn new(
        mark_id: ScalarMarkId,
        radii_um: Vec<f64>,
        permutations: usize,
        seed: u64,
        alpha: f64,
        limits: MarkWeightedKLimits,
    ) -> Result<Self, MarkWeightedKError> {
        if radii_um.is_empty() || radii_um.len() > limits.maximum_radii {
            return Err(MarkWeightedKError::InvalidConfig(
                "radius count must be within the configured positive bound".into(),
            ));
        }
        if radii_um.iter().any(|radius| {
            !radius.is_finite()
                || *radius <= 0.0
                || !(std::f64::consts::PI * radius * radius).is_finite()
        }) || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(MarkWeightedKError::InvalidConfig(
                "radii must be finite, positive, strictly increasing, and support finite pi-r-squared output".into(),
            ));
        }
        if permutations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || (permutations.saturating_add(1) as f64) * alpha < 1.0
        {
            return Err(MarkWeightedKError::InvalidConfig(
                "permutations/alpha do not support exact Monte Carlo inference".into(),
            ));
        }
        Ok(Self {
            mark_id,
            radii_um: radii_um.into_boxed_slice(),
            permutations,
            seed,
            alpha,
            limits,
        })
    }

    /// Exact continuous mark identity.
    pub fn mark_id(&self) -> &ScalarMarkId {
        &self.mark_id
    }
    /// Strictly increasing cumulative physical radii.
    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }
    /// Exact random-label replicate count.
    pub fn permutations(&self) -> usize {
        self.permutations
    }
    /// Deterministic base seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }
    /// ERL family-wise alpha.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
    /// Exact resource policy.
    pub fn limits(&self) -> MarkWeightedKLimits {
        self.limits
    }
}

/// Cumulative weighted-K availability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkWeightedKPointStatus {
    /// At least one center remains border-eligible.
    Available,
    /// No center remains border-eligible.
    NoEligibleCenters,
}

/// One cumulative physical radius of weighted and unweighted K.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkWeightedKPoint {
    /// Inclusive cumulative radius.
    pub radius_um: f64,
    /// Radius availability.
    pub status: MarkWeightedKPointStatus,
    /// Centers at least this far from the boundary.
    pub eligible_centers: usize,
    /// Cumulative ordered pairs from eligible centers.
    pub directed_pairs: usize,
    /// Compensated cumulative sum of `m_i m_j`.
    pub mark_product_sum: f64,
    /// Product sum divided by the squared global mean.
    pub normalized_weight_sum: f64,
    /// Cumulative product-weighted K.
    pub weighted_k: Option<f64>,
    /// Cumulative unweighted standard-border K baseline.
    pub unweighted_k: Option<f64>,
    /// Exact finite-row random-label expectation given the unweighted geometry.
    pub expected_random_label_weighted_k: Option<f64>,
    /// Homogeneous theoretical K, `pi r^2`.
    pub theoretical_k: f64,
    /// Whether this radius participates in the ERL family.
    pub inference_eligible: bool,
    /// Lower weighted-K ERL envelope.
    pub lower_weighted_k: Option<f64>,
    /// Upper weighted-K ERL envelope.
    pub upper_weighted_k: Option<f64>,
}

/// Exact shared geometry and retained-work telemetry.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkWeightedKGeometrySummary {
    /// Point rows.
    pub point_count: usize,
    /// Directed pairs retained through the maximum radius.
    pub directed_pair_count: usize,
    /// Geometry construction count.
    pub geometry_build_count: usize,
    /// Point/window geometry identity.
    pub geometry_digest: ContentDigest,
    /// Ordered directed pair-plan identity.
    pub pair_plan_digest: ContentDigest,
    /// Conservative retained-byte estimate including ERL workspace.
    pub estimated_storage_bytes: usize,
}

/// One ERL global result for weighted K.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkWeightedKComponentInference {
    /// Exact Monte Carlo global p-value.
    pub p_global: f64,
    /// Observed ERL depth.
    pub erl_depth: f64,
    /// Critical ERL depth.
    pub critical_depth: f64,
    /// Jointly eligible radii.
    pub eligible_radius_count: usize,
}

/// Complete-row random-labeling work and inference.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkWeightedKInferenceSummary {
    /// Exact null family.
    pub null_model: String,
    /// Atomic row moved by each permutation.
    pub permutation_unit: String,
    /// Two-sided alternative.
    pub alternative: String,
    /// Declared multiplicity family.
    pub multiplicity: String,
    /// Weighted-K ERL inference.
    pub weighted_k: Option<MarkWeightedKComponentInference>,
    /// Requested permutations.
    pub permutations_requested: usize,
    /// Attempted permutations.
    pub permutations_attempted: usize,
    /// Completed permutations.
    pub permutations_completed: usize,
    /// Base seed.
    pub seed: u64,
    /// Family-wise alpha.
    pub alpha: f64,
    /// Directed pair evaluations across null permutations.
    pub permutation_pair_evaluations: usize,
}

/// Complete typed cumulative mark-weighted K output.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkWeightedKResult {
    /// Exact continuous mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measurement status.
    pub measurement_status: MeasurementStatus,
    /// Fixed mark unit.
    pub unit: String,
    /// Exact physical coordinate frame.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Exact observation-window summary.
    pub window: ClassicalWindowSummary,
    /// Named pair weight.
    pub weight_function: String,
    /// Named edge correction.
    pub edge_correction: String,
    /// Global arithmetic mark mean.
    pub global_mark_mean: f64,
    /// Global population variance used for the constant guard.
    pub global_mark_population_variance: f64,
    /// Exact finite-row random-label mean normalized pair weight.
    pub expected_random_label_weight: f64,
    /// Shared geometry and retained-work telemetry.
    pub geometry: MarkWeightedKGeometrySummary,
    /// Configuration identity.
    pub configuration_digest: ContentDigest,
    /// Exact resource policy.
    pub limits: MarkWeightedKLimits,
    /// Radius-aligned cumulative curve.
    pub curve: Vec<MarkWeightedKPoint>,
    /// Complete-row random-label inference.
    pub inference: MarkWeightedKInferenceSummary,
}

/// Failure before one valid mark-weighted K result can be returned.
#[derive(Debug, Error)]
pub enum MarkWeightedKError {
    #[error("mark-weighted K resource limits must be positive")]
    InvalidResourceLimit,
    #[error("invalid mark-weighted K configuration: {0}")]
    InvalidConfig(String),
    #[error("mark-weighted K requires the declared finite continuous MarkTable column")]
    MissingContinuousMark,
    #[error("mark-weighted K requires nonzero global mark variance")]
    ZeroMarkVariance,
    #[error("mark-weighted K observation window is not frame-bound")]
    UnboundObservationWindow,
    #[error("mark-weighted K coordinate frames disagree")]
    CoordinateFrameMismatch,
    #[error("mark-weighted K has {observed} points; maximum is {maximum}")]
    PointLimitExceeded { observed: usize, maximum: usize },
    #[error("mark-weighted K plan requires more than {maximum} directed pairs")]
    DirectedPairLimitExceeded { maximum: usize },
    #[error("mark-weighted K null requires {required} pair evaluations; maximum is {maximum}")]
    PermutationPairLimitExceeded { required: usize, maximum: usize },
    #[error("mark-weighted K requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("mark-weighted K allocation failed")]
    AllocationFailed,
    #[error("mark-weighted K size arithmetic overflow")]
    SizeOverflow,
    #[error("mark-weighted K dependency failed: {reason}")]
    Dependency { reason: String },
}

use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::ContentDigest;
use thiserror::Error;

use crate::{ClassicalWindowSummary, ScalarMarkId};

/// Exact resource ceilings for one continuous mark-correlation workflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuousMarkCorrelationLimits {
    /// Maximum retained point rows.
    pub maximum_points: usize,
    /// Maximum physical radius values.
    pub maximum_radii: usize,
    /// Maximum directed geometry pairs retained once.
    pub maximum_directed_pairs: usize,
    /// Maximum directed-pair evaluations across all null permutations.
    pub maximum_permutation_pair_evaluations: usize,
    /// Maximum conservatively estimated retained bytes.
    pub maximum_retained_bytes: usize,
}

impl ContinuousMarkCorrelationLimits {
    /// Validate positive point, radius, pair, null-work, and memory ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_directed_pairs: usize,
        maximum_permutation_pair_evaluations: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, ContinuousMarkCorrelationError> {
        if [
            maximum_points,
            maximum_radii,
            maximum_directed_pairs,
            maximum_permutation_pair_evaluations,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(ContinuousMarkCorrelationError::InvalidResourceLimit);
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

/// Frozen continuous mark, radius axis, random-labeling controls, and resource policy.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuousMarkCorrelationConfig {
    pub(super) mark_id: ScalarMarkId,
    pub(super) radii_um: Box<[f64]>,
    pub(super) permutations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: ContinuousMarkCorrelationLimits,
}

impl ContinuousMarkCorrelationConfig {
    /// Validate one global-mean normalized product-correlation design.
    pub fn new(
        mark_id: ScalarMarkId,
        radii_um: Vec<f64>,
        permutations: usize,
        seed: u64,
        alpha: f64,
        limits: ContinuousMarkCorrelationLimits,
    ) -> Result<Self, ContinuousMarkCorrelationError> {
        if radii_um.is_empty() || radii_um.len() > limits.maximum_radii {
            return Err(ContinuousMarkCorrelationError::InvalidConfig(
                "radius count must be within the configured positive bound".into(),
            ));
        }
        if radii_um
            .iter()
            .any(|radius| !radius.is_finite() || *radius <= 0.0)
            || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ContinuousMarkCorrelationError::InvalidConfig(
                "radii must be finite, positive, and strictly increasing".into(),
            ));
        }
        if permutations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || (permutations.saturating_add(1) as f64) * alpha < 1.0
        {
            return Err(ContinuousMarkCorrelationError::InvalidConfig(
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

    /// Strictly increasing physical radius values.
    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }

    /// Exact random-labeling replicate count.
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

    /// Exact execution limits.
    pub fn limits(&self) -> ContinuousMarkCorrelationLimits {
        self.limits
    }
}

/// Radius-shell availability for normalized mark products.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuousMarkCorrelationPointStatus {
    /// At least one boundary-eligible directed pair occurs in the shell.
    Available,
    /// The shell contains no boundary-eligible directed pair.
    NoPairsInShell,
}

/// One radius of continuous mark correlation.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuousMarkCorrelationPoint {
    /// Inclusive shell upper edge.
    pub radius_um: f64,
    /// Exclusive shell lower edge, zero for the first radius.
    pub shell_lower_um: f64,
    /// Shell availability.
    pub status: ContinuousMarkCorrelationPointStatus,
    /// All directed boundary-eligible pairs in the shell.
    pub directed_pairs_in_shell: usize,
    /// Compensated sum of directed mark products in the shell.
    pub mark_product_sum_in_shell: f64,
    /// Mean pair product divided by the squared global mark mean.
    pub correlation: Option<f64>,
    /// Whether this radius participates in the joint ERL family.
    pub inference_eligible: bool,
    /// Lower ERL envelope when eligible.
    pub lower_correlation: Option<f64>,
    /// Upper ERL envelope when eligible.
    pub upper_correlation: Option<f64>,
}

/// Exact shared geometry and work telemetry.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuousMarkCorrelationGeometrySummary {
    /// Point rows.
    pub point_count: usize,
    /// Directed pairs retained through the maximum radius.
    pub directed_pair_count: usize,
    /// Geometry construction count for observed plus null evaluation.
    pub geometry_build_count: usize,
    /// Point/window geometry identity.
    pub geometry_digest: ContentDigest,
    /// Ordered directed pair-plan identity.
    pub pair_plan_digest: ContentDigest,
    /// Named edge correction.
    pub edge_correction: String,
    /// Conservative retained-byte estimate including ERL workspace.
    pub estimated_storage_bytes: usize,
}

/// One ERL global result for the correlation curve.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuousMarkCorrelationComponentInference {
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
pub struct ContinuousMarkCorrelationInferenceSummary {
    /// Exact null family.
    pub null_model: String,
    /// Atomic row moved by each permutation.
    pub permutation_unit: String,
    /// Two-sided alternative.
    pub alternative: String,
    /// Declared multiplicity family.
    pub multiplicity: String,
    /// Correlation ERL inference when at least one radius is eligible.
    pub correlation: Option<ContinuousMarkCorrelationComponentInference>,
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

/// Complete typed continuous mark-correlation output.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuousMarkCorrelationResult {
    /// Exact continuous mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measured or predicted status.
    pub measurement_status: MeasurementStatus,
    /// Fixed mark unit.
    pub unit: String,
    /// Exact physical frame shared by points and window.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Exact observation-window summary.
    pub window: ClassicalWindowSummary,
    /// Named formula and global-mean policy.
    pub normalization: String,
    /// Global arithmetic mark mean over all admitted rows.
    pub global_mark_mean: f64,
    /// Global population variance used for the constant-mark guard.
    pub global_mark_population_variance: f64,
    /// Exact finite-row random-label expectation under the fixed normalization.
    pub expected_random_label_correlation: f64,
    /// Shared geometry and work telemetry.
    pub geometry: ContinuousMarkCorrelationGeometrySummary,
    /// Configuration identity.
    pub configuration_digest: ContentDigest,
    /// Exact execution limits.
    pub limits: ContinuousMarkCorrelationLimits,
    /// Radius-aligned correlation output.
    pub curve: Vec<ContinuousMarkCorrelationPoint>,
    /// Random-labeling inference.
    pub inference: ContinuousMarkCorrelationInferenceSummary,
}

/// Failure before one valid continuous mark-correlation result can be returned.
#[derive(Debug, Error)]
pub enum ContinuousMarkCorrelationError {
    /// One or more resource ceilings are zero.
    #[error("continuous mark-correlation resource limits must be positive")]
    InvalidResourceLimit,
    /// Configuration is invalid.
    #[error("invalid continuous mark-correlation configuration: {0}")]
    InvalidConfig(String),
    /// Requested continuous column is absent.
    #[error(
        "continuous mark correlation requires the declared finite continuous MarkTable column"
    )]
    MissingContinuousMark,
    /// All continuous marks are exactly equal after f64 promotion.
    #[error("continuous mark correlation requires nonzero global mark variance")]
    ZeroMarkVariance,
    /// Observation window lacks a physical frame binding.
    #[error("continuous mark-correlation observation window is not frame-bound")]
    UnboundObservationWindow,
    /// Window and input frames disagree.
    #[error("continuous mark-correlation coordinate frames disagree")]
    CoordinateFrameMismatch,
    /// Point count exceeds the caller ceiling.
    #[error("continuous mark correlation has {observed} points; maximum is {maximum}")]
    PointLimitExceeded { observed: usize, maximum: usize },
    /// Directed pair plan exceeds the caller ceiling.
    #[error("continuous mark-correlation plan requires more than {maximum} directed pairs")]
    DirectedPairLimitExceeded { maximum: usize },
    /// Null pair work exceeds the caller ceiling.
    #[error("continuous mark-correlation null requires {required} pair evaluations; maximum is {maximum}")]
    PermutationPairLimitExceeded { required: usize, maximum: usize },
    /// Conservative retained bytes exceed the caller ceiling.
    #[error(
        "continuous mark correlation requires {required} retained bytes; maximum is {maximum}"
    )]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    /// Allocation failed.
    #[error("continuous mark-correlation allocation failed")]
    AllocationFailed,
    /// Checked arithmetic overflowed.
    #[error("continuous mark-correlation size arithmetic overflow")]
    SizeOverflow,
    /// An existing typed geometry or inference owner failed.
    #[error("continuous mark-correlation dependency failed: {reason}")]
    Dependency { reason: String },
}

use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::ContentDigest;
use thiserror::Error;

use crate::{ClassicalWindowSummary, ScalarMarkId};

/// Exact resource ceilings for one probability-weighted pair workflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProbabilityPairLimits {
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

impl ProbabilityPairLimits {
    /// Validate positive point, radius, pair, null-work, and memory ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_directed_pairs: usize,
        maximum_permutation_pair_evaluations: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, ProbabilityPairError> {
        if [
            maximum_points,
            maximum_radii,
            maximum_directed_pairs,
            maximum_permutation_pair_evaluations,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(ProbabilityPairError::InvalidResourceLimit);
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

/// Frozen probability column, radius axis, random-labeling controls, and resource policy.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbabilityPairConfig {
    pub(super) mark_id: ScalarMarkId,
    pub(super) radii_um: Box<[f64]>,
    pub(super) permutations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: ProbabilityPairLimits,
}

impl ProbabilityPairConfig {
    /// Validate one expected positive-positive connection design.
    pub fn new(
        mark_id: ScalarMarkId,
        radii_um: Vec<f64>,
        permutations: usize,
        seed: u64,
        alpha: f64,
        limits: ProbabilityPairLimits,
    ) -> Result<Self, ProbabilityPairError> {
        if radii_um.is_empty() || radii_um.len() > limits.maximum_radii {
            return Err(ProbabilityPairError::InvalidConfig(
                "radius count must be within the configured positive bound".into(),
            ));
        }
        if radii_um
            .iter()
            .any(|radius| !radius.is_finite() || *radius <= 0.0)
            || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ProbabilityPairError::InvalidConfig(
                "radii must be finite, positive, and strictly increasing".into(),
            ));
        }
        if permutations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || (permutations.saturating_add(1) as f64) * alpha < 1.0
        {
            return Err(ProbabilityPairError::InvalidConfig(
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

    /// Exact dense probability mark identity.
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
    pub fn limits(&self) -> ProbabilityPairLimits {
        self.limits
    }
}

/// Radius-shell availability for expected probability contributions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbabilityPairPointStatus {
    /// At least one boundary-eligible directed pair occurs in the shell.
    Available,
    /// The shell contains no boundary-eligible directed pair.
    NoPairsInShell,
}

/// One radius of expected positive-positive mark connection.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbabilityPairPoint {
    /// Inclusive shell upper edge.
    pub radius_um: f64,
    /// Exclusive shell lower edge, zero for the first radius.
    pub shell_lower_um: f64,
    /// Shell availability.
    pub status: ProbabilityPairPointStatus,
    /// All directed boundary-eligible pairs in the shell.
    pub directed_pairs_in_shell: usize,
    /// Sum of `p_i p_j` over eligible directed pairs in the shell.
    pub expected_positive_pairs_in_shell: f64,
    /// Expected positive-positive contribution per eligible directed pair.
    pub connection_probability: Option<f64>,
    /// Whether this radius participates in the joint ERL family.
    pub connection_inference_eligible: bool,
    /// Lower ERL envelope when eligible.
    pub lower_connection: Option<f64>,
    /// Upper ERL envelope when eligible.
    pub upper_connection: Option<f64>,
}

/// Exact shared geometry and work telemetry.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbabilityPairGeometrySummary {
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
    /// Conservative retained-byte estimate.
    pub estimated_storage_bytes: usize,
}

/// One ERL global result for the connection curve.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbabilityPairComponentInference {
    /// Exact Monte Carlo global p-value.
    pub p_global: f64,
    /// Observed ERL depth.
    pub erl_depth: f64,
    /// Critical ERL depth.
    pub critical_depth: f64,
    /// Jointly eligible radii.
    pub eligible_radius_count: usize,
}

/// Expected-contribution random-labeling work and inference.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbabilityPairInferenceSummary {
    /// Exact null family.
    pub null_model: String,
    /// Atomic row moved by each permutation.
    pub permutation_unit: String,
    /// Expected-contribution rather than sampled-label mode.
    pub probability_mode: String,
    /// Two-sided alternative.
    pub alternative: String,
    /// Declared multiplicity family.
    pub multiplicity: String,
    /// Connection ERL inference when at least one radius is eligible.
    pub connection: Option<ProbabilityPairComponentInference>,
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

/// Complete typed probability-weighted connection output.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbabilityPairResult {
    /// Exact probability mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measured or predicted status.
    pub measurement_status: MeasurementStatus,
    /// Exact physical frame shared by points and window.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Exact observation-window summary.
    pub window: ClassicalWindowSummary,
    /// Named formula and denominator policy.
    pub normalization: String,
    /// Sum of positive probabilities.
    pub effective_positive_mass: f64,
    /// Sum of complementary probabilities.
    pub effective_negative_mass: f64,
    /// Exact complete-row random-label expectation.
    pub expected_random_label_connection: f64,
    /// Shared geometry and work telemetry.
    pub geometry: ProbabilityPairGeometrySummary,
    /// Configuration identity.
    pub configuration_digest: ContentDigest,
    /// Exact execution limits.
    pub limits: ProbabilityPairLimits,
    /// Radius-aligned connection output.
    pub curve: Vec<ProbabilityPairPoint>,
    /// Random-labeling inference.
    pub inference: ProbabilityPairInferenceSummary,
}

/// Failure before one valid probability-pair result can be returned.
#[derive(Debug, Error)]
pub enum ProbabilityPairError {
    /// One or more resource ceilings are zero.
    #[error("probability pair resource limits must be positive")]
    InvalidResourceLimit,
    /// Configuration is invalid.
    #[error("invalid probability pair configuration: {0}")]
    InvalidConfig(String),
    /// Requested probability column is absent.
    #[error("probability pair requires the declared dense probability MarkTable column")]
    MissingProbabilityMark,
    /// Fewer than two rows carry nonzero positive probability mass.
    #[error("probability pair has insufficient expected positive-positive pair mass")]
    InsufficientEffectivePositivePairMass,
    /// Every row is certainly positive.
    #[error("probability pair has no effective negative mass")]
    NoEffectiveNegativeMass,
    /// Observation window lacks a physical frame binding.
    #[error("probability pair observation window is not frame-bound")]
    UnboundObservationWindow,
    /// Window and input frames disagree.
    #[error("probability pair coordinate frames disagree")]
    CoordinateFrameMismatch,
    /// Point count exceeds the caller ceiling.
    #[error("probability pair has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed point rows.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Directed pair plan exceeds the caller ceiling.
    #[error("probability pair plan requires more than {maximum} directed pairs")]
    DirectedPairLimitExceeded {
        /// Caller ceiling.
        maximum: usize,
    },
    /// Null pair work exceeds the caller ceiling.
    #[error("probability pair null requires {required} pair evaluations; maximum is {maximum}")]
    PermutationPairLimitExceeded {
        /// Required pair evaluations.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Conservative retained bytes exceed the caller ceiling.
    #[error("probability pair requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded {
        /// Required conservative bytes.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Allocation failed.
    #[error("probability pair allocation failed")]
    AllocationFailed,
    /// Checked arithmetic overflowed.
    #[error("probability pair size arithmetic overflow")]
    SizeOverflow,
    /// An existing typed geometry, inference, or codec owner failed.
    #[error("probability pair dependency failed: {reason}")]
    Dependency {
        /// Redacted deterministic reason.
        reason: String,
    },
}

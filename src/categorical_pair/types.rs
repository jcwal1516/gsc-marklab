use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::ContentDigest;
use thiserror::Error;

use crate::{ClassicalWindowSummary, ScalarMarkId};

/// Exact resource ceilings for one categorical pair workflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalPairLimits {
    /// Maximum retained point rows.
    pub maximum_points: usize,
    /// Maximum cumulative radius values.
    pub maximum_radii: usize,
    /// Maximum directed geometry pairs retained once.
    pub maximum_directed_pairs: usize,
    /// Maximum directed-pair evaluations across all null permutations.
    pub maximum_permutation_pair_evaluations: usize,
    /// Maximum conservatively estimated retained bytes.
    pub maximum_retained_bytes: usize,
}

impl CategoricalPairLimits {
    /// Validate positive point, radius, pair, permutation-work, and memory ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_directed_pairs: usize,
        maximum_permutation_pair_evaluations: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, CategoricalPairError> {
        if [
            maximum_points,
            maximum_radii,
            maximum_directed_pairs,
            maximum_permutation_pair_evaluations,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(CategoricalPairError::InvalidResourceLimit);
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

/// Frozen level pair, radius axis, random-labeling controls, and resource policy.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoricalPairConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) source_level: String,
    pub(super) target_level: String,
    pub(super) permutations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: CategoricalPairLimits,
}

impl CategoricalPairConfig {
    /// Validate one directed distinct-level categorical pair design.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        radii_um: Vec<f64>,
        source_level: impl Into<String>,
        target_level: impl Into<String>,
        permutations: usize,
        seed: u64,
        alpha: f64,
        limits: CategoricalPairLimits,
    ) -> Result<Self, CategoricalPairError> {
        if radii_um.is_empty() || radii_um.len() > limits.maximum_radii {
            return Err(CategoricalPairError::InvalidConfig(
                "radius count must be within the configured positive bound".into(),
            ));
        }
        if radii_um
            .iter()
            .any(|radius| !radius.is_finite() || *radius <= 0.0)
            || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(CategoricalPairError::InvalidConfig(
                "radii must be finite, positive, and strictly increasing".into(),
            ));
        }
        let source_level = source_level.into();
        let target_level = target_level.into();
        if !valid_level(&source_level)
            || !valid_level(&target_level)
            || source_level == target_level
        {
            return Err(CategoricalPairError::InvalidConfig(
                "source and target must be distinct bounded nonempty level labels".into(),
            ));
        }
        if permutations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || (permutations.saturating_add(1) as f64) * alpha < 1.0
        {
            return Err(CategoricalPairError::InvalidConfig(
                "permutations/alpha do not support exact Monte Carlo inference".into(),
            ));
        }
        Ok(Self {
            radii_um: radii_um.into_boxed_slice(),
            source_level,
            target_level,
            permutations,
            seed,
            alpha,
            limits,
        })
    }

    /// Strictly increasing physical radius values.
    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }

    /// Declared source level.
    pub fn source_level(&self) -> &str {
        &self.source_level
    }

    /// Declared target level.
    pub fn target_level(&self) -> &str {
        &self.target_level
    }

    /// Exact random-labeling replicate count.
    pub fn permutations(&self) -> usize {
        self.permutations
    }

    /// Deterministic seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// ERL family-wise alpha per named component family.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Exact execution limits.
    pub fn limits(&self) -> CategoricalPairLimits {
        self.limits
    }
}

/// Radius-shell mark-connection availability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CategoricalPairPointStatus {
    /// At least one boundary-eligible directed pair occurs in the shell.
    Available,
    /// The shell contains no boundary-eligible directed pair.
    NoPairsInShell,
}

/// One radius of directed categorical mark-connection and cumulative cross-K output.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoricalPairPoint {
    /// Inclusive cumulative radius and shell upper edge.
    pub radius_um: f64,
    /// Exclusive shell lower edge, zero for the first radius.
    pub shell_lower_um: f64,
    /// Mark-connection availability.
    pub status: CategoricalPairPointStatus,
    /// Source-level centers at least this far from the boundary.
    pub eligible_source_centers: usize,
    /// Cumulative directed source-to-target pairs within the radius.
    pub directed_source_target_pairs: usize,
    /// All directed boundary-eligible pairs in this shell.
    pub directed_pairs_in_shell: usize,
    /// Directed source-to-target pairs in this shell.
    pub source_target_pairs_in_shell: usize,
    /// Directed shell mark-connection probability.
    pub connection_probability: Option<f64>,
    /// Standard-border directed cross-K.
    pub cross_k: Option<f64>,
    /// Homogeneous independent-label theoretical cross-K.
    pub theoretical_cross_k: f64,
    /// Whether mark connection is jointly inference-eligible.
    pub connection_inference_eligible: bool,
    /// Lower mark-connection ERL envelope.
    pub lower_connection: Option<f64>,
    /// Upper mark-connection ERL envelope.
    pub upper_connection: Option<f64>,
    /// Whether cross-K is jointly inference-eligible.
    pub cross_k_inference_eligible: bool,
    /// Lower cross-K ERL envelope.
    pub lower_cross_k: Option<f64>,
    /// Upper cross-K ERL envelope.
    pub upper_cross_k: Option<f64>,
}

/// Exact shared geometry and work telemetry.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoricalPairGeometrySummary {
    /// Point rows.
    pub point_count: usize,
    /// Directed pairs retained once through the maximum radius.
    pub directed_pair_count: usize,
    /// Geometry was constructed exactly once and reused for all permutations.
    pub geometry_build_count: usize,
    /// Point/window geometry identity.
    pub geometry_digest: ContentDigest,
    /// Directed pair-plan identity.
    pub pair_plan_digest: ContentDigest,
    /// Edge correction.
    pub edge_correction: String,
    /// Conservative retained bytes.
    pub estimated_storage_bytes: usize,
}

/// One componentwise ERL global result.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoricalPairComponentInference {
    /// Exact Monte Carlo global p-value.
    pub p_global: f64,
    /// Observed ERL depth.
    pub erl_depth: f64,
    /// Critical ERL depth.
    pub critical_depth: f64,
    /// Jointly eligible radii.
    pub eligible_radius_count: usize,
}

/// Random-labeling work and separate componentwise ERL results.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoricalPairInferenceSummary {
    /// Exact null family.
    pub null_model: String,
    /// Complete categorical row is the atomic permutation unit.
    pub permutation_unit: String,
    /// Two-sided componentwise alternative.
    pub alternative: String,
    /// Each named curve is one simultaneous ERL family.
    pub multiplicity: String,
    /// Mark-connection inference.
    pub connection: Option<CategoricalPairComponentInference>,
    /// Directed cross-K inference.
    pub cross_k: Option<CategoricalPairComponentInference>,
    /// Requested permutations.
    pub permutations_requested: usize,
    /// Attempted permutations.
    pub permutations_attempted: usize,
    /// Completed permutations.
    pub permutations_completed: usize,
    /// Base seed.
    pub seed: u64,
    /// Family-wise alpha within each named component family.
    pub alpha: f64,
    /// Directed-pair evaluations across null permutations.
    pub permutation_pair_evaluations: usize,
}

/// Complete typed categorical pair output.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoricalPairResult {
    /// Fixed categorical mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measured or predicted status.
    pub measurement_status: MeasurementStatus,
    /// Exact physical frame shared by points and window.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Exact window summary.
    pub window: ClassicalWindowSummary,
    /// Declared source level.
    pub source_level: String,
    /// Declared target level.
    pub target_level: String,
    /// Total source rows.
    pub source_count: usize,
    /// Total target rows.
    pub target_count: usize,
    /// Random-label expectation for the ordered distinct-level connection probability.
    pub expected_random_label_connection: f64,
    /// Shared geometry and resource telemetry.
    pub geometry: CategoricalPairGeometrySummary,
    /// Configuration identity.
    pub configuration_digest: ContentDigest,
    /// Exact execution limits.
    pub limits: CategoricalPairLimits,
    /// Radius-aligned output.
    pub curve: Vec<CategoricalPairPoint>,
    /// Random-labeling inference.
    pub inference: CategoricalPairInferenceSummary,
}

/// Failure before one valid categorical pair result can be returned.
#[derive(Debug, Error)]
pub enum CategoricalPairError {
    /// One or more resource ceilings are zero.
    #[error("categorical pair resource limits must be positive")]
    InvalidResourceLimit,
    /// Configuration is invalid.
    #[error("invalid categorical pair configuration: {0}")]
    InvalidConfig(String),
    /// Typed MarkTable input is absent or does not expose the fixed category column.
    #[error(
        "categorical pair workflow requires the typed histologic_compartment MarkTable column"
    )]
    MissingCategoricalMark,
    /// Declared source or target label is not in the exact codebook.
    #[error("categorical pair level {level:?} is absent from the exact codebook")]
    UnknownLevel {
        /// Missing level.
        level: String,
    },
    /// A declared level has no retained rows.
    #[error("categorical pair level {level:?} has no retained rows")]
    EmptyLevel {
        /// Empty level.
        level: String,
    },
    /// Window has no physical frame binding.
    #[error("categorical pair observation window is not frame-bound")]
    UnboundObservationWindow,
    /// Window/input coordinate frames disagree.
    #[error("categorical pair coordinate frames disagree")]
    CoordinateFrameMismatch,
    /// Point count is outside the exact caller bound.
    #[error("categorical pair has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed rows.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Directed pair plan exceeds its caller ceiling.
    #[error("categorical pair plan requires more than {maximum} directed pairs")]
    DirectedPairLimitExceeded {
        /// Caller ceiling.
        maximum: usize,
    },
    /// Null pair evaluations exceed their caller ceiling.
    #[error("categorical pair null requires {required} pair evaluations; maximum is {maximum}")]
    PermutationPairLimitExceeded {
        /// Required evaluations.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Conservative retained bytes exceed their caller ceiling.
    #[error("categorical pair requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded {
        /// Required bytes.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Allocation failed.
    #[error("categorical pair allocation failed")]
    AllocationFailed,
    /// Checked arithmetic overflowed.
    #[error("categorical pair size arithmetic overflow")]
    SizeOverflow,
    /// Existing typed input, geometry, design, or ERL owner failed.
    #[error("categorical pair dependency failed: {reason}")]
    Dependency {
        /// Redacted deterministic reason.
        reason: String,
    },
}

fn valid_level(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

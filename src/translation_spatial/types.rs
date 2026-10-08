use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::classical::{ClassicalInferenceSummary, ClassicalNullDesign, ClassicalWindowSummary};
use crate::geom::window::ObservationWindowError;

/// Exact resource ceilings for one translation-corrected K/L execution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationSpatialLimits {
    /// Maximum retained point rows.
    pub maximum_points: usize,
    /// Maximum requested radii.
    pub maximum_radii: usize,
    /// Maximum unordered pairs inspected across observed and null patterns.
    pub maximum_pair_visits: usize,
    /// Maximum exact polygon-overlap evaluations.
    pub maximum_overlap_evaluations: usize,
    /// Maximum conservative segment-pair work across all overlap evaluations.
    pub maximum_overlap_candidate_work: usize,
    /// Maximum positions returned by any one Boolean intersection.
    pub maximum_overlap_output_vertices: usize,
    /// Maximum bounding-box proposals across all conditional-CSR patterns.
    pub maximum_csr_draws: usize,
    /// Maximum conservatively estimated retained bytes.
    pub maximum_retained_bytes: usize,
}

impl TranslationSpatialLimits {
    /// Validate positive caller-owned ceilings.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_pair_visits: usize,
        maximum_overlap_evaluations: usize,
        maximum_overlap_candidate_work: usize,
        maximum_overlap_output_vertices: usize,
        maximum_csr_draws: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, TranslationSpatialError> {
        let limits = Self {
            maximum_points,
            maximum_radii,
            maximum_pair_visits,
            maximum_overlap_evaluations,
            maximum_overlap_candidate_work,
            maximum_overlap_output_vertices,
            maximum_csr_draws,
            maximum_retained_bytes,
        };
        if [
            maximum_points,
            maximum_radii,
            maximum_pair_visits,
            maximum_overlap_evaluations,
            maximum_overlap_candidate_work,
            maximum_overlap_output_vertices,
            maximum_csr_draws,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(TranslationSpatialError::InvalidResourceLimit);
        }
        Ok(limits)
    }
}

/// Frozen radii, whole-pattern conditional-CSR design, and resource policy.
#[derive(Clone, Debug, PartialEq)]
pub struct TranslationSpatialConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: TranslationSpatialLimits,
}

impl TranslationSpatialConfig {
    /// Validate one exact translation-corrected homogeneous K/L configuration.
    pub fn new(
        radii_um: Vec<f64>,
        simulations: usize,
        seed: u64,
        alpha: f64,
        limits: TranslationSpatialLimits,
    ) -> Result<Self, TranslationSpatialError> {
        if radii_um.is_empty() || radii_um.len() > limits.maximum_radii {
            return Err(TranslationSpatialError::InvalidConfig {
                reason: "radius count must be within the configured positive bound".into(),
            });
        }
        if radii_um.iter().any(|radius| {
            !radius.is_finite()
                || *radius <= 0.0
                || !(std::f64::consts::PI * radius * radius).is_finite()
        }) || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(TranslationSpatialError::InvalidConfig {
                reason: "radii must be finite, positive, strictly increasing, and support finite pi-r-squared output".into(),
            });
        }
        if simulations == 0 || !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
            return Err(TranslationSpatialError::InvalidConfig {
                reason: "simulations must be positive and alpha must be finite in (0, 1)".into(),
            });
        }
        let curve_count = simulations
            .checked_add(1)
            .ok_or(TranslationSpatialError::SizeOverflow)?;
        if curve_count as f64 * alpha < 1.0 {
            return Err(TranslationSpatialError::InvalidConfig {
                reason: "(simulations + 1) * alpha must be at least one".into(),
            });
        }
        Ok(Self {
            radii_um: radii_um.into_boxed_slice(),
            simulations,
            seed,
            alpha,
            limits,
        })
    }

    /// Strictly increasing physical radii.
    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }

    /// Conditional-CSR simulation count.
    pub fn simulations(&self) -> usize {
        self.simulations
    }

    /// Deterministic seed root.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Family-wise ERL alpha.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Exact resource ceilings.
    pub fn limits(&self) -> TranslationSpatialLimits {
        self.limits
    }
}

/// Whole-workflow availability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationSpatialStatus {
    /// Observed and null K/L families are available.
    Available,
    /// Fewer than two points make the estimator undefined.
    InsufficientPoints,
}

/// One radius of exact translation-corrected homogeneous K/L output.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationKlPoint {
    /// Physical radius.
    pub radius_um: f64,
    /// Ordered distinct pairs contributed at this radius.
    pub ordered_pairs: usize,
    /// Unordered exact overlap evaluations contributing at this radius.
    pub overlap_evaluations: usize,
    /// Sum of pair-specific overlap areas, retained as a numerical diagnostic.
    pub translation_overlap_area_sum_um2: f64,
    /// Sum of both ordered `area(W) / overlap` weights.
    pub translation_weight_sum: f64,
    /// Translation-corrected homogeneous K.
    pub k: Option<f64>,
    /// Translation-corrected homogeneous L.
    pub l: Option<f64>,
    /// Homogeneous Poisson theoretical K.
    pub theoretical_k: f64,
    /// Homogeneous Poisson theoretical L.
    pub theoretical_l: f64,
    /// Lower ERL envelope for L.
    pub lower_l: Option<f64>,
    /// Upper ERL envelope for L.
    pub upper_l: Option<f64>,
}

/// Exact geometry and Boolean-overlap telemetry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationGeometrySummary {
    /// Retained point count.
    pub point_count: usize,
    /// Largest queried physical radius.
    pub maximum_radius_um: f64,
    /// Unordered pairs inspected for the observed pattern.
    pub observed_pair_visits: usize,
    /// Unordered pairs inspected across observed and null patterns.
    pub total_pair_visits: usize,
    /// Exact overlap calls for the observed pattern.
    pub observed_overlap_evaluations: usize,
    /// Exact overlap calls across observed and null patterns.
    pub total_overlap_evaluations: usize,
    /// Conservative segment-pair work charged across all overlap calls.
    pub total_overlap_candidate_work: usize,
    /// Maximum Boolean-intersection output positions observed.
    pub maximum_overlap_output_vertices_observed: usize,
    /// Conservative retained storage estimate.
    pub estimated_storage_bytes: usize,
    /// Exact point/window plan identity.
    pub logical_digest: String,
    /// Exact execution mode.
    pub execution_mode: String,
    /// Duplicate-coordinate policy.
    pub duplicate_policy: String,
    /// Owner of polygon translation overlap.
    pub overlap_owner: String,
    /// Pair traversal mode.
    pub pair_traversal: String,
}

/// Configuration identity persisted with the result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationConfigurationSummary {
    /// Radius count.
    pub radius_count: usize,
    /// Digest over radii, null controls, seed, alpha, and all ceilings.
    pub logical_digest: String,
    /// Exact caller ceilings.
    pub limits: TranslationSpatialLimits,
}

/// Typed translation-corrected K/L result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationSpatialResult {
    /// Source case identity.
    pub case_id: String,
    /// Source timepoint.
    pub timepoint: String,
    /// Fixed correction name.
    pub correction: String,
    /// Workflow availability.
    pub status: TranslationSpatialStatus,
    /// Exact observation window.
    pub window: ClassicalWindowSummary,
    /// Exact geometry and overlap work.
    pub geometry: TranslationGeometrySummary,
    /// Exact configuration identity.
    pub configuration: TranslationConfigurationSummary,
    /// Whole-pattern conditional-CSR design.
    pub null_design: ClassicalNullDesign,
    /// Radius-aligned observed curve and envelope.
    pub curve: Vec<TranslationKlPoint>,
    /// Simultaneous ERL inference, absent for insufficient points.
    pub inference: Option<ClassicalInferenceSummary>,
    /// Candidate draws consumed by all null patterns.
    pub csr_candidate_draws: usize,
}

/// Failure before a valid translation result can be produced.
#[derive(Debug, Error)]
pub enum TranslationSpatialError {
    /// One or more caller ceilings are zero.
    #[error("translation spatial resource limits must be positive")]
    InvalidResourceLimit,
    /// Configuration is invalid.
    #[error("invalid translation spatial configuration: {reason}")]
    InvalidConfig {
        /// Deterministic reason.
        reason: String,
    },
    /// Pattern arrays disagree or contain invalid rows.
    #[error("translation spatial pattern is invalid: {reason}")]
    InvalidPattern {
        /// Deterministic reason.
        reason: String,
    },
    /// Point count exceeds its ceiling.
    #[error("translation pattern has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed rows.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Pair traversal exceeds its ceiling.
    #[error("translation pair traversal used {observed} visits; maximum is {maximum}")]
    PairVisitLimitExceeded {
        /// Attempted visits.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Polygon-overlap call count exceeds its ceiling.
    #[error("translation overlap used {observed} evaluations; maximum is {maximum}")]
    OverlapEvaluationLimitExceeded {
        /// Attempted calls.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Conservative Boolean segment-pair work exceeds its ceiling.
    #[error("translation overlap requires {required} candidate work; maximum is {maximum}")]
    OverlapCandidateWorkLimitExceeded {
        /// Attempted cumulative work.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// An eligible pair has zero overlap measure.
    #[error("translation overlap is nonpositive for pair rows {left} and {right}")]
    NonPositiveOverlap {
        /// First point row.
        left: usize,
        /// Second point row.
        right: usize,
    },
    /// Conditional CSR cannot complete under its draw ceiling.
    #[error("translation conditional CSR failed: {reason}")]
    Csr {
        /// Redacted deterministic reason.
        reason: String,
    },
    /// Retained-memory estimate exceeds its ceiling.
    #[error("translation analysis requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded {
        /// Conservative requirement.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Exact window construction or overlap failed.
    #[error(transparent)]
    Window(#[from] ObservationWindowError),
    /// ERL inference rejected the finite curve family.
    #[error("translation global inference failed: {reason}")]
    Inference {
        /// Redacted deterministic reason.
        reason: String,
    },
    /// Allocation failed.
    #[error("translation analysis allocation failed")]
    AllocationFailed,
    /// Checked work or byte arithmetic overflowed.
    #[error("translation analysis size arithmetic overflow")]
    SizeOverflow,
}

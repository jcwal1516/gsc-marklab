use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::classical::{ClassicalInferenceSummary, ClassicalNullDesign, ClassicalWindowSummary};
use crate::geom::window::ObservationWindowError;

/// Exact resource ceilings for one isotropic K/L execution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicSpatialLimits {
    /// Maximum retained point rows.
    pub maximum_points: usize,
    /// Maximum requested radii.
    pub maximum_radii: usize,
    /// Maximum unordered pairs inspected across observed and null patterns.
    pub maximum_pair_visits: usize,
    /// Maximum directed visible-circle evaluations.
    pub maximum_visible_arc_evaluations: usize,
    /// Maximum boundary segment-circle tests.
    pub maximum_arc_segment_tests: usize,
    /// Maximum open-arc midpoint membership queries.
    pub maximum_arc_membership_queries: usize,
    /// Maximum conditional-CSR bounding-box proposals.
    pub maximum_csr_draws: usize,
    /// Maximum conservatively estimated retained and peak working bytes.
    pub maximum_retained_bytes: usize,
}

impl IsotropicSpatialLimits {
    /// Validate positive caller-owned ceilings.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_pair_visits: usize,
        maximum_visible_arc_evaluations: usize,
        maximum_arc_segment_tests: usize,
        maximum_arc_membership_queries: usize,
        maximum_csr_draws: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, IsotropicSpatialError> {
        let limits = Self {
            maximum_points,
            maximum_radii,
            maximum_pair_visits,
            maximum_visible_arc_evaluations,
            maximum_arc_segment_tests,
            maximum_arc_membership_queries,
            maximum_csr_draws,
            maximum_retained_bytes,
        };
        if [
            maximum_points,
            maximum_radii,
            maximum_pair_visits,
            maximum_visible_arc_evaluations,
            maximum_arc_segment_tests,
            maximum_arc_membership_queries,
            maximum_csr_draws,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(IsotropicSpatialError::InvalidResourceLimit);
        }
        Ok(limits)
    }
}

/// Frozen radii, whole-pattern conditional-CSR design, and resource policy.
#[derive(Clone, Debug, PartialEq)]
pub struct IsotropicSpatialConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: IsotropicSpatialLimits,
}

impl IsotropicSpatialConfig {
    /// Validate one exact isotropic homogeneous K/L configuration.
    pub fn new(
        radii_um: Vec<f64>,
        simulations: usize,
        seed: u64,
        alpha: f64,
        limits: IsotropicSpatialLimits,
    ) -> Result<Self, IsotropicSpatialError> {
        if radii_um.is_empty() || radii_um.len() > limits.maximum_radii {
            return Err(IsotropicSpatialError::InvalidConfig {
                reason: "radius count must be within the configured positive bound".into(),
            });
        }
        if radii_um.iter().any(|radius| {
            !radius.is_finite()
                || *radius <= 0.0
                || !(std::f64::consts::PI * radius * radius).is_finite()
        }) || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(IsotropicSpatialError::InvalidConfig {
                reason: "radii must be finite, positive, strictly increasing, and support finite pi-r-squared output".into(),
            });
        }
        if simulations == 0 || !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
            return Err(IsotropicSpatialError::InvalidConfig {
                reason: "simulations must be positive and alpha must be finite in (0, 1)".into(),
            });
        }
        let curve_count = simulations
            .checked_add(1)
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
        if curve_count as f64 * alpha < 1.0 {
            return Err(IsotropicSpatialError::InvalidConfig {
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

    /// Exact caller ceilings.
    pub fn limits(&self) -> IsotropicSpatialLimits {
        self.limits
    }
}

/// Whole-workflow availability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IsotropicSpatialStatus {
    /// Observed and null K/L families are available.
    Available,
    /// Fewer than two points make K/L undefined.
    InsufficientPoints,
}

/// One radius of exact isotropic K/L output.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicKlPoint {
    /// Physical radius.
    pub radius_um: f64,
    /// Directed distinct pairs within the cumulative radius.
    pub directed_pairs: usize,
    /// Directed visible-circle evaluations.
    pub visible_arc_evaluations: usize,
    /// Sum of directed visible-circumference fractions.
    pub visible_arc_fraction_sum: f64,
    /// Sum of reciprocal directed visible-circumference fractions.
    pub inverse_visible_arc_fraction_sum: f64,
    /// Isotropic homogeneous K.
    pub k: Option<f64>,
    /// Isotropic homogeneous L.
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

/// Exact point, pair, and visible-arc telemetry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicGeometrySummary {
    /// Retained point count.
    pub point_count: usize,
    /// Largest queried radius.
    pub maximum_radius_um: f64,
    /// Unordered pairs inspected for the observed pattern.
    pub observed_pair_visits: usize,
    /// Unordered pairs inspected across observed and null patterns.
    pub total_pair_visits: usize,
    /// Directed arc evaluations for the observed pattern.
    pub observed_visible_arc_evaluations: usize,
    /// Directed arc evaluations across observed and null patterns.
    pub total_visible_arc_evaluations: usize,
    /// Boundary segment-circle tests across all arc evaluations.
    pub total_arc_segment_tests: usize,
    /// Open-arc membership queries across all arc evaluations.
    pub total_arc_membership_queries: usize,
    /// Largest retained boundary-intersection angle count.
    pub maximum_boundary_intersection_angles: usize,
    /// Conservative retained and peak working bytes.
    pub estimated_storage_bytes: usize,
    /// Exact point/window plan identity.
    pub logical_digest: String,
    /// Exact execution mode.
    pub execution_mode: String,
    /// Duplicate-coordinate policy.
    pub duplicate_policy: String,
    /// Owner of visible-circle fractions.
    pub visible_arc_owner: String,
    /// Pair traversal mode.
    pub pair_traversal: String,
}

/// Configuration identity persisted with every result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicConfigurationSummary {
    /// Radius count.
    pub radius_count: usize,
    /// Digest over radii, null controls, seed, alpha, and all ceilings.
    pub logical_digest: String,
    /// Exact resource ceilings.
    pub limits: IsotropicSpatialLimits,
}

/// Typed isotropic K/L result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicSpatialResult {
    /// Source case identity.
    pub case_id: String,
    /// Source timepoint.
    pub timepoint: String,
    /// Fixed correction name.
    pub correction: String,
    /// Workflow availability.
    pub status: IsotropicSpatialStatus,
    /// Exact observation window.
    pub window: ClassicalWindowSummary,
    /// Exact geometry and work summary.
    pub geometry: IsotropicGeometrySummary,
    /// Configuration identity and limits.
    pub configuration: IsotropicConfigurationSummary,
    /// Whole-pattern conditional-CSR design.
    pub null_design: ClassicalNullDesign,
    /// Radius-aligned observed curve and envelope.
    pub curve: Vec<IsotropicKlPoint>,
    /// Simultaneous ERL inference, absent for insufficient points.
    pub inference: Option<ClassicalInferenceSummary>,
    /// Candidate draws consumed by all null patterns.
    pub csr_candidate_draws: usize,
}

/// Failure before a valid isotropic result can be produced.
#[derive(Debug, Error)]
pub enum IsotropicSpatialError {
    /// One or more caller ceilings are zero.
    #[error("isotropic spatial resource limits must be positive")]
    InvalidResourceLimit,
    /// Configuration is invalid.
    #[error("invalid isotropic spatial configuration: {reason}")]
    InvalidConfig {
        /// Deterministic reason.
        reason: String,
    },
    /// Pattern arrays or point geometry are invalid.
    #[error("isotropic spatial pattern is invalid: {reason}")]
    InvalidPattern {
        /// Deterministic reason.
        reason: String,
    },
    /// Point count exceeds its ceiling.
    #[error("isotropic pattern has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed rows.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Pair visits exceed their ceiling.
    #[error("isotropic pair traversal used {observed} visits; maximum is {maximum}")]
    PairVisitLimitExceeded {
        /// Attempted visits.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Directed arc evaluations exceed their ceiling.
    #[error("isotropic correction used {observed} arc evaluations; maximum is {maximum}")]
    VisibleArcEvaluationLimitExceeded {
        /// Attempted evaluations.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Boundary segment-circle tests exceed their ceiling.
    #[error("isotropic correction requires {required} segment tests; maximum is {maximum}")]
    ArcSegmentTestLimitExceeded {
        /// Attempted cumulative work.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Open-arc membership queries exceed their ceiling.
    #[error("isotropic correction requires more than {maximum} arc membership queries")]
    ArcMembershipQueryLimitExceeded {
        /// Caller ceiling.
        maximum: usize,
    },
    /// A directed pair has zero visible circumference measure.
    #[error("isotropic visible arc is nonpositive for directed pair rows {center} -> {neighbor}")]
    NonPositiveVisibleArc {
        /// Center row.
        center: usize,
        /// Neighbor row.
        neighbor: usize,
    },
    /// Conditional CSR cannot complete under its ceiling.
    #[error("isotropic conditional CSR failed: {reason}")]
    Csr {
        /// Redacted deterministic reason.
        reason: String,
    },
    /// Conservative memory estimate exceeds its ceiling.
    #[error("isotropic analysis requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded {
        /// Conservative requirement.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Exact observation-window geometry failed.
    #[error(transparent)]
    Window(#[from] ObservationWindowError),
    /// ERL inference rejected the finite family.
    #[error("isotropic global inference failed: {reason}")]
    Inference {
        /// Redacted deterministic reason.
        reason: String,
    },
    /// Allocation failed.
    #[error("isotropic analysis allocation failed")]
    AllocationFailed,
    /// Checked arithmetic overflowed.
    #[error("isotropic analysis size arithmetic overflow")]
    SizeOverflow,
}

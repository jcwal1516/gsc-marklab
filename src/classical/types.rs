use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::geom::window::ObservationWindowError;

/// Exact resource ceilings for one classical spatial workflow execution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicalSpatialLimits {
    /// Maximum retained point rows.
    pub maximum_points: usize,
    /// Maximum radius values.
    pub maximum_radii: usize,
    /// Maximum ordered neighbor visits across observed and simulated patterns.
    pub maximum_pair_visits: usize,
    /// Maximum bounding-box candidates drawn across all CSR simulations.
    pub maximum_csr_draws: usize,
    /// Maximum conservatively estimated retained bytes.
    pub maximum_retained_bytes: usize,
}

impl ClassicalSpatialLimits {
    /// Validate positive execution ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_pair_visits: usize,
        maximum_csr_draws: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, ClassicalSpatialError> {
        if [
            maximum_points,
            maximum_radii,
            maximum_pair_visits,
            maximum_csr_draws,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(ClassicalSpatialError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_radii,
            maximum_pair_visits,
            maximum_csr_draws,
            maximum_retained_bytes,
        })
    }
}

/// Frozen radii, conditional-CSR design, and resource policy.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassicalSpatialConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: ClassicalSpatialLimits,
}

impl ClassicalSpatialConfig {
    /// Validate one standard-border homogeneous K/L configuration.
    pub fn new(
        radii_um: Vec<f64>,
        simulations: usize,
        seed: u64,
        alpha: f64,
        limits: ClassicalSpatialLimits,
    ) -> Result<Self, ClassicalSpatialError> {
        if radii_um.is_empty() || radii_um.len() > limits.maximum_radii {
            return Err(ClassicalSpatialError::InvalidConfig {
                reason: "radius count must be within the configured positive bound".into(),
            });
        }
        if radii_um
            .iter()
            .any(|radius| !radius.is_finite() || *radius <= 0.0)
            || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ClassicalSpatialError::InvalidConfig {
                reason: "radii must be finite, positive, and strictly increasing".into(),
            });
        }
        if simulations == 0 || !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
            return Err(ClassicalSpatialError::InvalidConfig {
                reason: "simulations must be positive and alpha must be finite in (0, 1)".into(),
            });
        }
        let curve_count = simulations
            .checked_add(1)
            .ok_or(ClassicalSpatialError::SizeOverflow)?;
        if curve_count as f64 * alpha < 1.0 {
            return Err(ClassicalSpatialError::InvalidConfig {
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

    /// Strictly increasing physical radius values.
    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }

    /// Exact conditional-CSR simulation count.
    pub fn simulations(&self) -> usize {
        self.simulations
    }

    /// Explicit deterministic seed namespace root.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Family-wise ERL alpha.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Exact execution resource ceilings.
    pub fn limits(&self) -> ClassicalSpatialLimits {
        self.limits
    }
}

/// Supported location-process null.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassicalNullModel {
    /// Homogeneous binomial point process conditional on observed point count.
    HomogeneousCsrConditionalOnCount,
}

/// Exchangeable unit for the location-process null.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassicalRandomizationUnit {
    /// Resample every location jointly; cells are not independent population replicates.
    WholeLocationPattern,
}

/// Fully explicit null-design summary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicalNullDesign {
    /// Fixed null family.
    pub null_model: ClassicalNullModel,
    /// Exact resampling unit.
    pub randomization_unit: ClassicalRandomizationUnit,
    /// Observed point count held fixed.
    pub conditioned_point_count: usize,
    /// Exact simulation count.
    pub simulations: usize,
    /// Deterministic seed.
    pub seed: u64,
    /// ERL family-wise alpha.
    pub alpha: f64,
}

/// Availability of the whole classical workflow.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassicalSpatialStatus {
    /// Observed curve and global inference are available.
    Available,
    /// Fewer than two retained points make K/L undefined.
    InsufficientPoints,
    /// Observed K/L exists but no radius is eligible across every CSR curve.
    InsufficientInferenceSupport,
}

/// Availability of one border-corrected radius.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KlPointStatus {
    /// At least one center is border-eligible.
    Available,
    /// No center is far enough from the boundary.
    NoEligibleCenters,
}

/// One radius of homogeneous border-corrected K/L output.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HomogeneousKlPoint {
    /// Physical radius.
    pub radius_um: f64,
    /// Per-radius availability.
    pub status: KlPointStatus,
    /// Centers whose boundary distance is at least the radius.
    pub eligible_centers: usize,
    /// Ordered distinct pairs contributed by eligible centers.
    pub ordered_pairs: usize,
    /// Border-corrected K, absent when unavailable.
    pub k: Option<f64>,
    /// Border-corrected L, absent when unavailable.
    pub l: Option<f64>,
    /// Homogeneous Poisson theoretical K.
    pub theoretical_k: f64,
    /// Homogeneous Poisson theoretical L.
    pub theoretical_l: f64,
    /// Whether observed and every simulated L value are available.
    pub inference_eligible: bool,
    /// Lower ERL envelope for L when inference-eligible.
    pub lower_l: Option<f64>,
    /// Upper ERL envelope for L when inference-eligible.
    pub upper_l: Option<f64>,
}

/// Bounded window summary embedded in the strict result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicalWindowSummary {
    /// Physical coordinate unit inherited from the supported pattern input.
    pub coordinate_unit: String,
    /// Explicit version-one frame label for existing physical x/y pattern coordinates.
    pub coordinate_frame: String,
    /// Boundary-inclusive membership policy.
    pub membership_policy: String,
    /// Physical window area.
    pub area_um2: f64,
    /// Total boundary length.
    pub perimeter_um: f64,
    /// Physical [min_x, min_y, max_x, max_y] bounds.
    pub bounds_um: [f64; 4],
    /// Polygon components.
    pub component_count: usize,
    /// Holes.
    pub hole_count: usize,
    /// Rings.
    pub ring_count: usize,
    /// Positions including closures.
    pub vertex_count: usize,
    /// Canonical geometry digest.
    pub logical_digest: String,
}

/// Exact geometry-plan telemetry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicalGeometrySummary {
    /// Retained point count.
    pub point_count: usize,
    /// Exact maximum queried radius.
    pub maximum_radius_um: f64,
    /// Pair visits for the observed pattern.
    pub observed_pair_visits: usize,
    /// Pair visits across observed and simulated patterns.
    pub total_pair_visits: usize,
    /// Conservative plan storage estimate.
    pub estimated_storage_bytes: usize,
    /// Exact point/window plan identity.
    pub logical_digest: String,
    /// Execution is always exact in version one.
    pub execution_mode: String,
    /// Duplicate-coordinate policy.
    pub duplicate_policy: String,
    /// Owner of exact boundary-distance queries.
    pub boundary_distance_owner: String,
    /// Exact pair traversal mode.
    pub pair_traversal: String,
}

/// Configuration identity and exact resource policy embedded in every result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicalConfigurationSummary {
    /// Number of strictly increasing radius values.
    pub radius_count: usize,
    /// Digest over radii, null controls, seed, alpha, and every resource ceiling.
    pub logical_digest: String,
    /// Exact resource ceilings used by the computation.
    pub limits: ClassicalSpatialLimits,
}

/// ERL global inference summary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicalInferenceSummary {
    /// Exact Monte Carlo global p-value.
    pub p_global: f64,
    /// Observed normalized extreme-rank-length depth.
    pub erl_depth: f64,
    /// Critical ERL depth used for the envelope.
    pub critical_depth: f64,
    /// Exact simulation count.
    pub simulations: usize,
    /// Number of inference-eligible radii.
    pub eligible_radius_count: usize,
}

/// Deterministic typed output of the classical spatial workflow.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicalSpatialResult {
    /// Source case identifier.
    pub case_id: String,
    /// Source timepoint.
    pub timepoint: String,
    /// Whole-workflow availability.
    pub status: ClassicalSpatialStatus,
    /// Exact window summary.
    pub window: ClassicalWindowSummary,
    /// Exact geometry-plan summary.
    pub geometry: ClassicalGeometrySummary,
    /// Exact configuration identity and resource policy.
    pub configuration: ClassicalConfigurationSummary,
    /// Explicit conditional-CSR design.
    pub null_design: ClassicalNullDesign,
    /// Radius-aligned observed curve and envelope.
    pub curve: Vec<HomogeneousKlPoint>,
    /// ERL inference, absent when unavailable.
    pub inference: Option<ClassicalInferenceSummary>,
    /// Candidate draws consumed by all CSR simulations.
    pub csr_candidate_draws: usize,
}

/// Failure before a valid classical result can be committed.
#[derive(Debug, Error)]
pub enum ClassicalSpatialError {
    /// One or more resource limits are zero.
    #[error("classical spatial resource limits must be positive")]
    InvalidResourceLimit,
    /// Configuration is scientifically or numerically invalid.
    #[error("invalid classical spatial configuration: {reason}")]
    InvalidConfig {
        /// Deterministic reason.
        reason: String,
    },
    /// Pattern coordinate/validity arrays disagree.
    #[error("classical pattern coordinate/validity arrays must match the row count")]
    PatternShapeMismatch,
    /// Point count exceeds the caller ceiling.
    #[error("classical pattern has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed points.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// A coordinate is non-finite.
    #[error("classical point row {row} is non-finite")]
    NonFinitePoint {
        /// Invalid row.
        row: usize,
    },
    /// A retained point is outside the exact window.
    #[error("classical point row {row} is outside the observation window")]
    PointOutsideWindow {
        /// Invalid row.
        row: usize,
    },
    /// Duplicate coordinates violate the version-one simple-point contract.
    #[error("classical point rows {first_row} and {second_row} have duplicate coordinates")]
    DuplicatePoint {
        /// First row.
        first_row: usize,
        /// Duplicate row.
        second_row: usize,
    },
    /// Pair traversal exceeded the caller ceiling.
    #[error("classical pair traversal used {observed} visits; maximum is {maximum}")]
    PairVisitLimitExceeded {
        /// Attempted visits.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// CSR rejection sampling exceeded the caller ceiling.
    #[error("conditional CSR used more than {maximum} candidate draws")]
    CsrDrawLimitExceeded {
        /// Caller ceiling.
        maximum: usize,
    },
    /// Conservative retained-memory estimate exceeds the caller ceiling.
    #[error("classical analysis requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded {
        /// Conservative required bytes.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Fallible allocation failed.
    #[error("classical analysis allocation failed")]
    AllocationFailed,
    /// Checked count or byte arithmetic overflowed.
    #[error("classical analysis size arithmetic overflow")]
    SizeOverflow,
    /// Exact observation-window construction or query failed.
    #[error(transparent)]
    Window(#[from] ObservationWindowError),
    /// Existing geometry substrate failed.
    #[error("classical geometry failed: {reason}")]
    Geometry {
        /// Redacted reason.
        reason: String,
    },
    /// Existing ERL inference rejected the curve family.
    #[error("classical global inference failed: {reason}")]
    Inference {
        /// Redacted reason.
        reason: String,
    },
}

impl ClassicalSpatialError {
    pub(super) fn geometry(error: impl std::fmt::Display) -> Self {
        Self::Geometry {
            reason: error.to_string(),
        }
    }
}

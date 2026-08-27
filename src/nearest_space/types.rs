use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ClassicalNullDesign, ClassicalWindowSummary};

/// Exact resource ceilings for one nearest/empty-space analysis.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpaceLimits {
    /// Maximum retained event rows.
    pub maximum_points: usize,
    /// Maximum physical radii.
    pub maximum_radii: usize,
    /// Maximum retained deterministic probes.
    pub maximum_probes: usize,
    /// Maximum event and probe nearest-neighbour queries across observed and null patterns.
    pub maximum_nearest_queries: usize,
    /// Maximum conditional-CSR bounding-box candidate draws.
    pub maximum_csr_draws: usize,
    /// Maximum conservatively estimated retained bytes.
    pub maximum_retained_bytes: usize,
}

impl NearestSpaceLimits {
    /// Validate positive explicit resource ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_probes: usize,
        maximum_nearest_queries: usize,
        maximum_csr_draws: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, NearestSpaceError> {
        if [
            maximum_points,
            maximum_radii,
            maximum_probes,
            maximum_nearest_queries,
            maximum_csr_draws,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(NearestSpaceError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_radii,
            maximum_probes,
            maximum_nearest_queries,
            maximum_csr_draws,
            maximum_retained_bytes,
        })
    }
}

/// Frozen radii, probe grid, conditional-CSR design, and work policy.
#[derive(Clone, Debug, PartialEq)]
pub struct NearestSpaceConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) probe_grid: [usize; 2],
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) j_denominator_epsilon: f64,
    pub(super) limits: NearestSpaceLimits,
}

impl NearestSpaceConfig {
    /// Validate one exact reduced-sample F/G/J configuration.
    pub fn new(
        radii_um: Vec<f64>,
        probe_grid: [usize; 2],
        simulations: usize,
        seed: u64,
        alpha: f64,
        j_denominator_epsilon: f64,
        limits: NearestSpaceLimits,
    ) -> Result<Self, NearestSpaceError> {
        if radii_um.is_empty() || radii_um.len() > limits.maximum_radii {
            return Err(NearestSpaceError::InvalidConfig {
                reason: "radius count must be within the configured positive bound".into(),
            });
        }
        if radii_um
            .iter()
            .any(|radius| !radius.is_finite() || *radius <= 0.0)
            || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(NearestSpaceError::InvalidConfig {
                reason: "radii must be finite, positive, and strictly increasing".into(),
            });
        }
        let requested_probes = probe_grid[0]
            .checked_mul(probe_grid[1])
            .ok_or(NearestSpaceError::SizeOverflow)?;
        if probe_grid.contains(&0) || requested_probes > limits.maximum_probes {
            return Err(NearestSpaceError::InvalidConfig {
                reason: "probe-grid dimensions must be positive and within the probe limit".into(),
            });
        }
        if simulations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || (simulations.saturating_add(1) as f64) * alpha < 1.0
        {
            return Err(NearestSpaceError::InvalidConfig {
                reason: "simulations/alpha do not support exact Monte Carlo inference".into(),
            });
        }
        if !j_denominator_epsilon.is_finite()
            || j_denominator_epsilon <= 0.0
            || j_denominator_epsilon >= 1.0
        {
            return Err(NearestSpaceError::InvalidConfig {
                reason: "J denominator epsilon must be finite in (0, 1)".into(),
            });
        }
        Ok(Self {
            radii_um: radii_um.into_boxed_slice(),
            probe_grid,
            simulations,
            seed,
            alpha,
            j_denominator_epsilon,
            limits,
        })
    }

    /// Strictly increasing physical radius values.
    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }

    /// Requested fixed cell-centred probe grid `[x, y]`.
    pub fn probe_grid(&self) -> [usize; 2] {
        self.probe_grid
    }

    /// Exact conditional-CSR simulation count.
    pub fn simulations(&self) -> usize {
        self.simulations
    }

    /// Deterministic seed namespace root.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Family-wise ERL alpha.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Positive floor below which `1-F` makes J unavailable.
    pub fn j_denominator_epsilon(&self) -> f64 {
        self.j_denominator_epsilon
    }

    /// Exact resource ceilings.
    pub fn limits(&self) -> NearestSpaceLimits {
        self.limits
    }
}

/// Whole-workflow availability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NearestSpaceStatus {
    /// At least one component has exact observed and global-inference support.
    Available,
    /// Fewer than two events make G and J unavailable.
    InsufficientEvents,
    /// Observed curves exist but no component is jointly eligible across null patterns.
    InsufficientInferenceSupport,
}

/// Availability of F or G at one radius.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionPointStatus {
    /// A positive reduced-sample denominator exists.
    Available,
    /// No probe or event center is at least this far from the window boundary.
    NoEligibleCenters,
    /// G requires at least two distinct events.
    InsufficientEvents,
}

/// Availability of J at one radius.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JPointStatus {
    /// Both component estimates and a stable denominator exist.
    Available,
    /// F or G is unavailable at this radius.
    MissingComponent,
    /// `1-F` does not exceed the declared positive floor.
    DenominatorTooSmall,
}

/// One physical radius of reduced-sample F, G, and J output.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpacePoint {
    /// Physical radius.
    pub radius_um: f64,
    /// F availability.
    pub f_status: DistributionPointStatus,
    /// G availability.
    pub g_status: DistributionPointStatus,
    /// J availability.
    pub j_status: JPointStatus,
    /// Boundary-eligible deterministic probes.
    pub eligible_probes: usize,
    /// Eligible probes whose nearest event lies within the radius.
    pub probes_with_event_within_radius: usize,
    /// Boundary-eligible event centers.
    pub eligible_event_centers: usize,
    /// Eligible events whose nearest other event lies within the radius.
    pub events_with_neighbor_within_radius: usize,
    /// Border-corrected empty-space CDF.
    pub f: Option<f64>,
    /// Border-corrected event nearest-neighbour CDF.
    pub g: Option<f64>,
    /// `J=(1-G)/(1-F)` when finite and identified.
    pub j: Option<f64>,
    /// Whether F is jointly inference-eligible.
    pub f_inference_eligible: bool,
    /// F lower ERL envelope.
    pub lower_f: Option<f64>,
    /// F upper ERL envelope.
    pub upper_f: Option<f64>,
    /// Whether G is jointly inference-eligible.
    pub g_inference_eligible: bool,
    /// G lower ERL envelope.
    pub lower_g: Option<f64>,
    /// G upper ERL envelope.
    pub upper_g: Option<f64>,
    /// Whether J is jointly inference-eligible.
    pub j_inference_eligible: bool,
    /// J lower ERL envelope.
    pub lower_j: Option<f64>,
    /// J upper ERL envelope.
    pub upper_j: Option<f64>,
}

/// Deterministic probe-plan identity and discretization metadata.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpaceProbeSummary {
    /// Requested cell-centred rectangular grid.
    pub requested_grid: [usize; 2],
    /// Probes retained by exact closed-window membership.
    pub retained_probe_count: usize,
    /// Grid spacing in physical x/y units.
    pub spacing_um: [f64; 2],
    /// Half-cell diagonal: maximum location displacement inside one grid cell.
    pub maximum_location_error_um: f64,
    /// Exact fixed probe-plan identifier.
    pub logical_digest: String,
    /// Probe construction policy.
    pub method: String,
}

/// Exact geometry and work telemetry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpaceGeometrySummary {
    /// Retained event count.
    pub point_count: usize,
    /// Maximum queried radius.
    pub maximum_radius_um: f64,
    /// Exact observed-plus-null nearest queries.
    pub nearest_query_count: usize,
    /// Conservative peak retained-byte estimate.
    pub estimated_storage_bytes: usize,
    /// Exact point/window geometry identity.
    pub logical_digest: String,
    /// Nearest-neighbour owner.
    pub nearest_neighbor_owner: String,
    /// Edge policy.
    pub edge_correction: String,
}

/// Exact configuration identity and limits.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpaceConfigurationSummary {
    /// Radius count.
    pub radius_count: usize,
    /// Positive J denominator floor.
    pub j_denominator_epsilon: f64,
    /// Digest over radii, probe, null, seed, and resource controls.
    pub logical_digest: String,
    /// Exact execution limits.
    pub limits: NearestSpaceLimits,
}

/// ERL inference for one curve component.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpaceComponentInference {
    /// Exact Monte Carlo global p-value.
    pub p_global: f64,
    /// Observed ERL depth.
    pub erl_depth: f64,
    /// Critical ERL depth.
    pub critical_depth: f64,
    /// Jointly eligible radius count.
    pub eligible_radius_count: usize,
}

/// Separate F, G, and J global inference; unsupported components remain absent.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpaceInferenceSummary {
    /// Empty-space inference.
    pub f: Option<NearestSpaceComponentInference>,
    /// Event nearest-neighbour inference.
    pub g: Option<NearestSpaceComponentInference>,
    /// J inference only where every observed/null denominator is stable.
    pub j: Option<NearestSpaceComponentInference>,
}

/// Complete typed F/G/J output.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpaceResult {
    /// Source case identifier.
    pub case_id: String,
    /// Source timepoint.
    pub timepoint: String,
    /// Whole-workflow availability.
    pub status: NearestSpaceStatus,
    /// Exact observation-window summary.
    pub window: ClassicalWindowSummary,
    /// Exact probe-plan summary.
    pub probes: NearestSpaceProbeSummary,
    /// Exact geometry/work summary.
    pub geometry: NearestSpaceGeometrySummary,
    /// Configuration identity and resource limits.
    pub configuration: NearestSpaceConfigurationSummary,
    /// Whole-pattern conditional-CSR null design.
    pub null_design: ClassicalNullDesign,
    /// Radius-aligned F/G/J curves and envelopes.
    pub curve: Vec<NearestSpacePoint>,
    /// Separate componentwise global inference.
    pub inference: NearestSpaceInferenceSummary,
    /// Candidate draws consumed by conditional CSR.
    pub csr_candidate_draws: usize,
}

/// Failure before a valid nearest/empty-space result can be committed.
#[derive(Debug, Error)]
pub enum NearestSpaceError {
    /// One or more resource limits are zero.
    #[error("nearest-space resource limits must be positive")]
    InvalidResourceLimit,
    /// Configuration is scientifically or numerically invalid.
    #[error("invalid nearest-space configuration: {reason}")]
    InvalidConfig {
        /// Deterministic reason.
        reason: String,
    },
    /// Pattern shape is invalid.
    #[error("nearest-space pattern coordinate/validity arrays disagree")]
    PatternShapeMismatch,
    /// Point count exceeds the caller ceiling.
    #[error("nearest-space pattern has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed rows.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Probe construction retained no points inside the exact window.
    #[error("nearest-space deterministic probe grid retains no points inside the window")]
    NoProbesInsideWindow,
    /// Nearest-query work exceeds the caller ceiling.
    #[error("nearest-space requires more than {maximum} nearest queries")]
    NearestQueryLimitExceeded {
        /// Caller ceiling.
        maximum: usize,
    },
    /// Conditional-CSR candidate draws exceed the caller ceiling.
    #[error("nearest-space conditional CSR used more than {maximum} candidate draws")]
    CsrDrawLimitExceeded {
        /// Caller ceiling.
        maximum: usize,
    },
    /// Conservative retained-memory estimate exceeds the caller ceiling.
    #[error("nearest-space requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded {
        /// Conservative requirement.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Allocation failed.
    #[error("nearest-space allocation failed")]
    AllocationFailed,
    /// Checked arithmetic overflowed.
    #[error("nearest-space size arithmetic overflow")]
    SizeOverflow,
    /// Canonical geometry/index/window processing failed.
    #[error("nearest-space geometry failed: {reason}")]
    Geometry {
        /// Redacted deterministic reason.
        reason: String,
    },
    /// Existing ERL inference rejected a curve family.
    #[error("nearest-space global inference failed: {reason}")]
    Inference {
        /// Redacted deterministic reason.
        reason: String,
    },
}

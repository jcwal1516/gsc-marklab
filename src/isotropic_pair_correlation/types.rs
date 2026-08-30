use serde::{Deserialize, Serialize};

use crate::{
    ClassicalInferenceSummary, ClassicalNullDesign, ClassicalWindowSummary, IsotropicSpatialError,
    IsotropicSpatialLimits, PairCorrelationKernel, PairCorrelationPointStatus,
};

/// Explicit radii, Epanechnikov bandwidth, null design, and visible-arc ceilings.
#[derive(Clone, Debug, PartialEq)]
pub struct IsotropicPairCorrelationConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) bandwidth_um: f64,
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: IsotropicSpatialLimits,
}

impl IsotropicPairCorrelationConfig {
    /// Validate one exact isotropic homogeneous pair-correlation design.
    pub fn new(
        radii_um: Vec<f64>,
        bandwidth_um: f64,
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
        if !bandwidth_um.is_finite() || bandwidth_um <= 0.0 {
            return Err(IsotropicSpatialError::InvalidConfig {
                reason: "pair bandwidth must be positive and finite".into(),
            });
        }
        if radii_um.iter().any(|radius| {
            !radius.is_finite()
                || *radius <= bandwidth_um
                || !(*radius + bandwidth_um).is_finite()
                || !(std::f64::consts::TAU * radius).is_finite()
        }) || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(IsotropicSpatialError::InvalidConfig {
                reason: "radii must be finite, strictly increasing, greater than bandwidth, and support finite normalization".into(),
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
            bandwidth_um,
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
    /// Positive Epanechnikov half-bandwidth.
    pub fn bandwidth_um(&self) -> f64 {
        self.bandwidth_um
    }
    /// Whole-pattern conditional-CSR count.
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
    /// Exact caller-owned ceilings.
    pub fn limits(&self) -> IsotropicSpatialLimits {
        self.limits
    }
}

/// One radius of exact isotropic homogeneous pair correlation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicPairCorrelationPoint {
    /// Physical kernel centre.
    pub radius_um: f64,
    /// Typed compact-support availability.
    pub status: PairCorrelationPointStatus,
    /// Directed distinct pairs within strict compact support.
    pub directed_pairs_in_support: usize,
    /// Directed visible-circle evaluations represented at this radius.
    pub visible_arc_evaluations: usize,
    /// Sum of represented directed visible fractions.
    pub visible_arc_fraction_sum: f64,
    /// Sum of kernel weights divided by each directed visible fraction.
    pub inverse_visible_arc_weighted_kernel_sum: f64,
    /// Exact isotropic homogeneous pair correlation.
    pub g: Option<f64>,
    /// Homogeneous Poisson theoretical value.
    pub theoretical_g: f64,
    /// Whether this radius participates in simultaneous ERL inference.
    pub inference_eligible: bool,
    /// Lower simultaneous ERL envelope.
    pub lower_g: Option<f64>,
    /// Upper simultaneous ERL envelope.
    pub upper_g: Option<f64>,
}

/// Exact geometry, pair traversal, and visible-arc telemetry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicPairCorrelationGeometrySummary {
    /// Retained point count.
    pub point_count: usize,
    /// Largest radius plus the explicit half-bandwidth.
    pub maximum_query_radius_um: f64,
    /// Unordered observed-pattern pairs inspected.
    pub observed_pair_visits: usize,
    /// Unordered pairs inspected across observed and null patterns.
    pub total_pair_visits: usize,
    /// Directed observed-pattern arc evaluations.
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
    /// Exact point/window geometry identity.
    pub logical_digest: String,
    /// Exact rather than approximate execution mode.
    pub execution_mode: String,
    /// Duplicate-coordinate handling policy.
    pub duplicate_policy: String,
    /// Canonical visible-circle implementation identity.
    pub visible_arc_owner: String,
    /// Pair traversal and directed-weight semantics.
    pub pair_traversal: String,
}

/// Configuration identity retained with one isotropic g result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicPairCorrelationConfigurationSummary {
    /// Number of requested radii.
    pub radius_count: usize,
    /// Digest over radii, bandwidth, null controls, seed, alpha, and ceilings.
    pub logical_digest: String,
    /// Exact caller-owned ceilings.
    pub limits: IsotropicSpatialLimits,
}

/// Typed exact isotropic homogeneous pair-correlation result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicPairCorrelationResult {
    /// Source case identity.
    pub case_id: String,
    /// Source timepoint identity.
    pub timepoint: String,
    /// Fixed edge-correction name.
    pub correction: String,
    /// Fixed compact-support kernel.
    pub kernel: PairCorrelationKernel,
    /// Positive physical half-bandwidth.
    pub bandwidth_um: f64,
    /// Exact observation-window summary.
    pub window: ClassicalWindowSummary,
    /// Exact geometry and arc work.
    pub geometry: IsotropicPairCorrelationGeometrySummary,
    /// Exact configuration identity.
    pub configuration: IsotropicPairCorrelationConfigurationSummary,
    /// Whole-pattern conditional-CSR design.
    pub null_design: ClassicalNullDesign,
    /// Fixed population-randomization unit.
    pub randomization_unit: String,
    /// Radius-aligned observed curve and simultaneous envelope.
    pub curve: Vec<IsotropicPairCorrelationPoint>,
    /// Simultaneous ERL inference when at least one radius is jointly eligible.
    pub inference: Option<ClassicalInferenceSummary>,
    /// Bounding-box proposals consumed by all null patterns.
    pub csr_candidate_draws: usize,
}

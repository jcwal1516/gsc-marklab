use serde::{Deserialize, Serialize};

use crate::{
    ClassicalInferenceSummary, ClassicalNullDesign, ClassicalWindowSummary, PairCorrelationKernel,
    PairCorrelationPointStatus, TranslationSpatialError, TranslationSpatialLimits,
};

/// Explicit radii, Epanechnikov bandwidth, null design, and exact translation ceilings.
#[derive(Clone, Debug, PartialEq)]
pub struct TranslationPairCorrelationConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) bandwidth_um: f64,
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: TranslationSpatialLimits,
}

impl TranslationPairCorrelationConfig {
    /// Validate one exact translation-corrected homogeneous pair-correlation design.
    pub fn new(
        radii_um: Vec<f64>,
        bandwidth_um: f64,
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
        if !bandwidth_um.is_finite() || bandwidth_um <= 0.0 {
            return Err(TranslationSpatialError::InvalidConfig {
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
            return Err(TranslationSpatialError::InvalidConfig {
                reason: "radii must be finite, strictly increasing, greater than bandwidth, and support finite normalization".into(),
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

    /// Exact caller-owned resource ceilings.
    pub fn limits(&self) -> TranslationSpatialLimits {
        self.limits
    }
}

/// One radius of exact translation-corrected homogeneous pair correlation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationPairCorrelationPoint {
    /// Physical radius at the kernel center.
    pub radius_um: f64,
    /// Typed availability; empty compact support is unavailable rather than zero.
    pub status: PairCorrelationPointStatus,
    /// Ordered distinct pairs within strict compact support.
    pub ordered_pairs_in_support: usize,
    /// Unordered exact overlap calls represented at this radius.
    pub overlap_evaluations: usize,
    /// Sum of pair-specific overlap areas represented at this radius.
    pub translation_overlap_area_sum_um2: f64,
    /// Sum of Epanechnikov weights times both ordered `area(W) / overlap` factors.
    pub translation_weighted_kernel_sum: f64,
    /// Translation-corrected homogeneous pair correlation.
    pub g: Option<f64>,
    /// Homogeneous Poisson theoretical pair correlation.
    pub theoretical_g: f64,
    /// Whether this radius participates in the simultaneous ERL family.
    pub inference_eligible: bool,
    /// Lower simultaneous ERL envelope.
    pub lower_g: Option<f64>,
    /// Upper simultaneous ERL envelope.
    pub upper_g: Option<f64>,
}

/// Exact geometry, pair traversal, and Boolean-overlap telemetry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationPairCorrelationGeometrySummary {
    /// Retained point count.
    pub point_count: usize,
    /// Largest radius plus the explicit kernel half-bandwidth.
    pub maximum_query_radius_um: f64,
    /// Unordered pairs inspected for the observed pattern.
    pub observed_pair_visits: usize,
    /// Unordered pairs inspected across observed and null patterns.
    pub total_pair_visits: usize,
    /// Exact polygon-overlap calls for the observed pattern.
    pub observed_overlap_evaluations: usize,
    /// Exact polygon-overlap calls across observed and null patterns.
    pub total_overlap_evaluations: usize,
    /// Conservative segment-pair work charged across all overlap calls.
    pub total_overlap_candidate_work: usize,
    /// Maximum positions returned by any Boolean intersection.
    pub maximum_overlap_output_vertices_observed: usize,
    /// Conservative retained-storage estimate.
    pub estimated_storage_bytes: usize,
    /// Exact point/window geometry identity.
    pub logical_digest: String,
    /// Exact rather than approximate execution mode.
    pub execution_mode: String,
    /// Duplicate-coordinate handling policy.
    pub duplicate_policy: String,
    /// Canonical exact overlap implementation identity.
    pub overlap_owner: String,
    /// Pair traversal and directed-weight semantics.
    pub pair_traversal: String,
}

/// Configuration identity retained with one translation-corrected g result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationPairCorrelationConfigurationSummary {
    /// Number of requested radii.
    pub radius_count: usize,
    /// Digest over radii, bandwidth, null controls, seed, alpha, and all ceilings.
    pub logical_digest: String,
    /// Exact caller-owned ceilings.
    pub limits: TranslationSpatialLimits,
}

/// Typed exact translation-corrected homogeneous pair-correlation result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationPairCorrelationResult {
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
    /// Exact geometry and overlap work.
    pub geometry: TranslationPairCorrelationGeometrySummary,
    /// Exact configuration identity.
    pub configuration: TranslationPairCorrelationConfigurationSummary,
    /// Whole-pattern conditional-CSR design.
    pub null_design: ClassicalNullDesign,
    /// Fixed population-randomization unit.
    pub randomization_unit: String,
    /// Radius-aligned observed curve and simultaneous envelope.
    pub curve: Vec<TranslationPairCorrelationPoint>,
    /// Simultaneous ERL inference when at least one radius is jointly eligible.
    pub inference: Option<ClassicalInferenceSummary>,
    /// Bounding-box proposals consumed by all null patterns.
    pub csr_candidate_draws: usize,
}

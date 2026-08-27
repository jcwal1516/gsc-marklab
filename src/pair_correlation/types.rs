use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ClassicalSpatialLimits, ClassicalWindowSummary};

/// Frozen compact-support kernel for homogeneous pair correlation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PairCorrelationKernel {
    /// Symmetric Epanechnikov density kernel on `[-1, 1]`.
    Epanechnikov,
}

/// Explicit radius, bandwidth, conditional-CSR controls, and resource ceilings.
#[derive(Clone, Debug, PartialEq)]
pub struct HomogeneousPairCorrelationConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) bandwidth_um: f64,
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: ClassicalSpatialLimits,
}

impl HomogeneousPairCorrelationConfig {
    /// Validate one Epanechnikov standard-border homogeneous g design.
    pub fn new(
        radii_um: Vec<f64>,
        bandwidth_um: f64,
        simulations: usize,
        seed: u64,
        alpha: f64,
        limits: ClassicalSpatialLimits,
    ) -> Result<Self, HomogeneousPairCorrelationError> {
        if radii_um.is_empty() || radii_um.len() > limits.maximum_radii {
            return Err(HomogeneousPairCorrelationError::InvalidConfig(
                "radius count must be within the configured positive bound".into(),
            ));
        }
        if !bandwidth_um.is_finite() || bandwidth_um <= 0.0 {
            return Err(HomogeneousPairCorrelationError::InvalidConfig(
                "bandwidth must be positive and finite".into(),
            ));
        }
        if radii_um.iter().any(|radius| {
            !radius.is_finite()
                || *radius <= bandwidth_um
                || !(*radius + bandwidth_um).is_finite()
                || !(2.0 * std::f64::consts::PI * radius).is_finite()
        }) || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(HomogeneousPairCorrelationError::InvalidConfig(
                "radii must be finite, strictly increasing, greater than bandwidth, and support finite normalization".into(),
            ));
        }
        if simulations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || (simulations.saturating_add(1) as f64) * alpha < 1.0
        {
            return Err(HomogeneousPairCorrelationError::InvalidConfig(
                "simulations/alpha do not support exact Monte Carlo inference".into(),
            ));
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

    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }

    pub fn bandwidth_um(&self) -> f64 {
        self.bandwidth_um
    }

    pub fn simulations(&self) -> usize {
        self.simulations
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    pub fn limits(&self) -> ClassicalSpatialLimits {
        self.limits
    }
}

/// Availability of one kernel-smoothed radius.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PairCorrelationPointStatus {
    Available,
    NoEligibleCenters,
    NoPairsInKernelSupport,
}

/// One standard-border homogeneous pair-correlation estimate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PairCorrelationPoint {
    pub radius_um: f64,
    pub status: PairCorrelationPointStatus,
    pub eligible_centers: usize,
    pub directed_pairs_in_support: usize,
    pub kernel_weight_sum: f64,
    pub g: Option<f64>,
    pub theoretical_g: f64,
    pub inference_eligible: bool,
    pub lower_g: Option<f64>,
    pub upper_g: Option<f64>,
}

/// One two-sided simultaneous conditional-CSR result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HomogeneousPairCorrelationInference {
    pub p_global: f64,
    pub erl_depth: f64,
    pub critical_depth: f64,
    pub eligible_radius_count: usize,
}

/// Complete homogeneous pair-correlation result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HomogeneousPairCorrelationResult {
    pub case_id: String,
    pub timepoint: String,
    pub window: ClassicalWindowSummary,
    pub kernel: PairCorrelationKernel,
    pub bandwidth_um: f64,
    pub edge_correction: String,
    pub null_model: String,
    pub randomization_unit: String,
    pub geometry_digest: String,
    pub geometry_build_count: usize,
    pub observed_pair_visits: usize,
    pub total_pair_visits: usize,
    pub estimated_storage_bytes: usize,
    pub configuration_digest: String,
    pub limits: ClassicalSpatialLimits,
    pub simulations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub csr_candidate_draws: usize,
    pub curve: Vec<PairCorrelationPoint>,
    pub inference: Option<HomogeneousPairCorrelationInference>,
}

/// Failure before a finite typed pair-correlation result exists.
#[derive(Debug, Error)]
pub enum HomogeneousPairCorrelationError {
    #[error("invalid homogeneous pair-correlation configuration: {0}")]
    InvalidConfig(String),
    #[error("pair-correlation pattern coordinate/validity arrays disagree")]
    PatternShapeMismatch,
    #[error("pair-correlation pair visits exceeded maximum {maximum}")]
    PairVisitLimitExceeded { maximum: usize },
    #[error("pair correlation requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("pair-correlation allocation failed")]
    AllocationFailed,
    #[error("pair-correlation size arithmetic overflow")]
    SizeOverflow,
    #[error("pair-correlation dependency failed: {0}")]
    Dependency(String),
}

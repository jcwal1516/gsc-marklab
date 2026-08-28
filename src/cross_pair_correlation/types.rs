use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CategoricalPairLimits, ClassicalWindowSummary, PairCorrelationKernel,
    PairCorrelationPointStatus,
};

#[derive(Clone, Debug, PartialEq)]
pub struct CategoricalCrossPairCorrelationConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) bandwidth_um: f64,
    pub(super) source_level: String,
    pub(super) target_level: String,
    pub(super) permutations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: CategoricalPairLimits,
}

impl CategoricalCrossPairCorrelationConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        radii_um: Vec<f64>,
        bandwidth_um: f64,
        source_level: impl Into<String>,
        target_level: impl Into<String>,
        permutations: usize,
        seed: u64,
        alpha: f64,
        limits: CategoricalPairLimits,
    ) -> Result<Self, CategoricalCrossPairCorrelationError> {
        if radii_um.is_empty()
            || radii_um.len() > limits.maximum_radii
            || !bandwidth_um.is_finite()
            || bandwidth_um <= 0.0
            || radii_um.iter().any(|radius| {
                !radius.is_finite()
                    || *radius <= bandwidth_um
                    || !(*radius + bandwidth_um).is_finite()
                    || !(2.0 * std::f64::consts::PI * radius).is_finite()
            })
            || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(CategoricalCrossPairCorrelationError::InvalidConfig(
                "radii and bandwidth do not define finite ordered compact support".into(),
            ));
        }
        let source_level = source_level.into();
        let target_level = target_level.into();
        if source_level.is_empty()
            || target_level.is_empty()
            || source_level == target_level
            || source_level.len() > 128
            || target_level.len() > 128
            || source_level.trim() != source_level
            || target_level.trim() != target_level
            || source_level.chars().any(char::is_control)
            || target_level.chars().any(char::is_control)
        {
            return Err(CategoricalCrossPairCorrelationError::InvalidConfig(
                "source and target must be distinct bounded labels".into(),
            ));
        }
        if permutations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || (permutations.saturating_add(1) as f64) * alpha < 1.0
        {
            return Err(CategoricalCrossPairCorrelationError::InvalidConfig(
                "permutations/alpha do not support exact inference".into(),
            ));
        }
        Ok(Self {
            radii_um: radii_um.into_boxed_slice(),
            bandwidth_um,
            source_level,
            target_level,
            permutations,
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

    pub fn source_level(&self) -> &str {
        &self.source_level
    }

    pub fn target_level(&self) -> &str {
        &self.target_level
    }

    pub fn permutations(&self) -> usize {
        self.permutations
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    pub fn limits(&self) -> CategoricalPairLimits {
        self.limits
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalCrossPairCorrelationPoint {
    pub radius_um: f64,
    pub status: PairCorrelationPointStatus,
    pub eligible_source_centers: usize,
    pub directed_source_target_pairs_in_support: usize,
    pub kernel_weight_sum: f64,
    pub cross_g: Option<f64>,
    pub theoretical_cross_g: f64,
    pub inference_eligible: bool,
    pub lower_cross_g: Option<f64>,
    pub upper_cross_g: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalCrossPairCorrelationInference {
    pub null_model: String,
    pub permutation_unit: String,
    pub p_global: Option<f64>,
    pub erl_depth: Option<f64>,
    pub critical_depth: Option<f64>,
    pub eligible_radius_count: usize,
    pub permutations_completed: usize,
    pub seed: u64,
    pub alpha: f64,
    pub permutation_pair_evaluations: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalCrossPairCorrelationResult {
    pub mark_id: String,
    pub measurement_status: String,
    pub coordinate_frame_id: String,
    pub window: ClassicalWindowSummary,
    pub source_level: String,
    pub target_level: String,
    pub source_count: usize,
    pub target_count: usize,
    pub kernel: PairCorrelationKernel,
    pub bandwidth_um: f64,
    pub edge_correction: String,
    pub geometry_digest: String,
    pub pair_plan_digest: String,
    pub directed_pair_count: usize,
    pub geometry_build_count: usize,
    pub estimated_storage_bytes: usize,
    pub configuration_digest: String,
    pub curve: Vec<CategoricalCrossPairCorrelationPoint>,
    pub inference: CategoricalCrossPairCorrelationInference,
}

#[derive(Debug, Error)]
pub enum CategoricalCrossPairCorrelationError {
    #[error("invalid categorical cross-pair-correlation configuration: {0}")]
    InvalidConfig(String),
    #[error("categorical cross-g requires the typed histologic_compartment MarkTable column")]
    MissingCategoricalMark,
    #[error("categorical cross-g level {0:?} is absent or empty")]
    MissingLevel(String),
    #[error("categorical cross-g input and window frames disagree")]
    CoordinateFrameMismatch,
    #[error("categorical cross-g work exceeds a configured resource ceiling")]
    ResourceLimitExceeded,
    #[error("categorical cross-g requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("categorical cross-g allocation failed")]
    AllocationFailed,
    #[error("categorical cross-g size arithmetic overflow")]
    SizeOverflow,
    #[error("categorical cross-g dependency failed: {0}")]
    Dependency(String),
}

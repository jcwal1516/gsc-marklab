use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ClassicalWindowSummary, InhomogeneousIntensitySummary, InhomogeneousSpatialConfig,
    InhomogeneousSpatialError, InhomogeneousSpatialInference, InhomogeneousSpatialLimits,
    PairCorrelationKernel, PairCorrelationPointStatus,
};

#[derive(Clone, Debug, PartialEq)]
pub struct InhomogeneousCategoricalCrossPairCorrelationConfig {
    pub(super) intensity: InhomogeneousSpatialConfig,
    pub(super) pair_bandwidth_um: f64,
    pub(super) source_level: String,
    pub(super) target_level: String,
}

impl InhomogeneousCategoricalCrossPairCorrelationConfig {
    pub fn new(
        intensity: InhomogeneousSpatialConfig,
        pair_bandwidth_um: f64,
        source_level: impl Into<String>,
        target_level: impl Into<String>,
    ) -> Result<Self, InhomogeneousCategoricalCrossPairCorrelationError> {
        if !pair_bandwidth_um.is_finite()
            || pair_bandwidth_um <= 0.0
            || intensity.radii_um().iter().any(|radius| {
                *radius <= pair_bandwidth_um || !(*radius + pair_bandwidth_um).is_finite()
            })
        {
            return Err(
                InhomogeneousCategoricalCrossPairCorrelationError::InvalidConfig(
                    "pair bandwidth must be finite, positive, below every radius, and have finite support"
                        .into(),
                ),
            );
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
            return Err(
                InhomogeneousCategoricalCrossPairCorrelationError::InvalidConfig(
                    "source and target must be distinct bounded labels".into(),
                ),
            );
        }
        Ok(Self {
            intensity,
            pair_bandwidth_um,
            source_level,
            target_level,
        })
    }

    pub fn intensity_config(&self) -> &InhomogeneousSpatialConfig {
        &self.intensity
    }

    pub fn pair_bandwidth_um(&self) -> f64 {
        self.pair_bandwidth_um
    }

    pub fn source_level(&self) -> &str {
        &self.source_level
    }

    pub fn target_level(&self) -> &str {
        &self.target_level
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousCategoricalCrossPairCorrelationPoint {
    pub radius_um: f64,
    pub status: PairCorrelationPointStatus,
    pub eligible_source_centers: usize,
    pub directed_source_target_pairs_in_support: usize,
    pub inverse_intensity_kernel_sum: f64,
    pub eligible_source_inverse_intensity_sum: f64,
    pub cross_g: Option<f64>,
    pub theoretical_cross_g: f64,
    pub inference_eligible: bool,
    pub lower_cross_g: Option<f64>,
    pub upper_cross_g: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousCategoricalCrossPairCorrelationResult {
    pub mark_id: String,
    pub measurement_status: String,
    pub coordinate_frame_id: String,
    pub window: ClassicalWindowSummary,
    pub source_level: String,
    pub target_level: String,
    pub source_count: usize,
    pub target_count: usize,
    pub source_rows: Vec<usize>,
    pub target_rows: Vec<usize>,
    pub source_intensity: InhomogeneousIntensitySummary,
    pub target_intensity: InhomogeneousIntensitySummary,
    pub kernel: PairCorrelationKernel,
    pub pair_bandwidth_um: f64,
    pub intensity_bandwidth_um: f64,
    pub edge_correction: String,
    pub configuration_digest: String,
    pub observed_pair_visits: usize,
    pub total_pair_visits: usize,
    pub intensity_evaluations: usize,
    pub estimated_storage_bytes: usize,
    pub limits: InhomogeneousSpatialLimits,
    pub curve: Vec<InhomogeneousCategoricalCrossPairCorrelationPoint>,
    pub inference: InhomogeneousSpatialInference,
}

#[derive(Debug, Error)]
pub enum InhomogeneousCategoricalCrossPairCorrelationError {
    #[error("invalid inhomogeneous categorical cross-g configuration: {0}")]
    InvalidConfig(String),
    #[error("inhomogeneous categorical cross-g requires the typed histologic_compartment MarkTable column")]
    MissingCategoricalMark,
    #[error("inhomogeneous categorical cross-g level {0:?} is absent")]
    MissingLevel(String),
    #[error("inhomogeneous categorical cross-g level {level:?} has {count} rows; at least two are required for leave-one-out intensity")]
    SparseLevel { level: String, count: usize },
    #[error("inhomogeneous categorical cross-g input and window frames disagree")]
    CoordinateFrameMismatch,
    #[error("inhomogeneous categorical cross-g requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("inhomogeneous categorical cross-g size arithmetic overflow")]
    SizeOverflow,
    #[error("inhomogeneous categorical cross-g allocation failed")]
    AllocationFailed,
    #[error("inhomogeneous categorical cross-g dependency failed: {0}")]
    Dependency(String),
    #[error(transparent)]
    Inhomogeneous(#[from] InhomogeneousSpatialError),
}

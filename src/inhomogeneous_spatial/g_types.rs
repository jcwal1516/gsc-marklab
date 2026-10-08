use serde::{Deserialize, Serialize};

use crate::{
    ClassicalWindowSummary, InhomogeneousIntensitySummary, InhomogeneousSpatialConfig,
    InhomogeneousSpatialError, InhomogeneousSpatialInference, InhomogeneousSpatialLimits,
    PairCorrelationKernel, PairCorrelationPointStatus,
};

#[derive(Clone, Debug, PartialEq)]
pub struct InhomogeneousPairCorrelationConfig {
    pub(super) intensity: InhomogeneousSpatialConfig,
    pub(super) pair_bandwidth_um: f64,
}

impl InhomogeneousPairCorrelationConfig {
    pub fn new(
        intensity: InhomogeneousSpatialConfig,
        pair_bandwidth_um: f64,
    ) -> Result<Self, InhomogeneousSpatialError> {
        if !pair_bandwidth_um.is_finite()
            || pair_bandwidth_um <= 0.0
            || intensity.radii_um().iter().any(|radius| {
                *radius <= pair_bandwidth_um || !(*radius + pair_bandwidth_um).is_finite()
            })
        {
            return Err(InhomogeneousSpatialError::InvalidConfig(
                "pair bandwidth must be finite, positive, below every radius, and have finite support"
                    .into(),
            ));
        }
        Ok(Self {
            intensity,
            pair_bandwidth_um,
        })
    }

    pub fn intensity_config(&self) -> &InhomogeneousSpatialConfig {
        &self.intensity
    }

    pub fn pair_bandwidth_um(&self) -> f64 {
        self.pair_bandwidth_um
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousPairCorrelationPoint {
    pub radius_um: f64,
    pub status: PairCorrelationPointStatus,
    pub eligible_centers: usize,
    pub directed_pairs_in_support: usize,
    pub inverse_intensity_kernel_sum: f64,
    pub eligible_center_inverse_intensity_sum: f64,
    pub g: Option<f64>,
    pub theoretical_g: f64,
    pub inference_eligible: bool,
    pub lower_g: Option<f64>,
    pub upper_g: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousPairCorrelationResult {
    pub case_id: String,
    pub timepoint: String,
    pub window: ClassicalWindowSummary,
    pub intensity: InhomogeneousIntensitySummary,
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
    pub curve: Vec<InhomogeneousPairCorrelationPoint>,
    pub inference: InhomogeneousSpatialInference,
}

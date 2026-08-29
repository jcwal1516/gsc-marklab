use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ClassicalWindowSummary;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousSpatialLimits {
    pub maximum_points: usize,
    pub maximum_radii: usize,
    pub maximum_probes: usize,
    pub maximum_intensity_evaluations: usize,
    pub maximum_pair_visits: usize,
    pub maximum_null_draws: usize,
    pub maximum_retained_bytes: usize,
}

impl InhomogeneousSpatialLimits {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_probes: usize,
        maximum_intensity_evaluations: usize,
        maximum_pair_visits: usize,
        maximum_null_draws: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, InhomogeneousSpatialError> {
        if [
            maximum_points,
            maximum_radii,
            maximum_probes,
            maximum_intensity_evaluations,
            maximum_pair_visits,
            maximum_null_draws,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(InhomogeneousSpatialError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_radii,
            maximum_probes,
            maximum_intensity_evaluations,
            maximum_pair_visits,
            maximum_null_draws,
            maximum_retained_bytes,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InhomogeneousSpatialConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) bandwidth_um: f64,
    pub(super) integration_grid: [usize; 2],
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) minimum_intensity_per_um2: f64,
    pub(super) limits: InhomogeneousSpatialLimits,
}

impl InhomogeneousSpatialConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        radii_um: Vec<f64>,
        bandwidth_um: f64,
        integration_grid: [usize; 2],
        simulations: usize,
        seed: u64,
        alpha: f64,
        minimum_intensity_per_um2: f64,
        limits: InhomogeneousSpatialLimits,
    ) -> Result<Self, InhomogeneousSpatialError> {
        let probe_count = integration_grid[0]
            .checked_mul(integration_grid[1])
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
        if radii_um.is_empty()
            || radii_um.len() > limits.maximum_radii
            || radii_um.iter().any(|radius| {
                !radius.is_finite()
                    || *radius <= 0.0
                    || !(std::f64::consts::PI * radius * radius).is_finite()
            })
            || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
            || !bandwidth_um.is_finite()
            || bandwidth_um <= 0.0
            || !(2.0 * std::f64::consts::PI * bandwidth_um * bandwidth_um).is_finite()
            || integration_grid.contains(&0)
            || probe_count > limits.maximum_probes
            || simulations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || (simulations.saturating_add(1) as f64) * alpha < 1.0
            || !minimum_intensity_per_um2.is_finite()
            || minimum_intensity_per_um2 <= 0.0
        {
            return Err(InhomogeneousSpatialError::InvalidConfig(
                "radii, bandwidth, grid, inference, or intensity floor are invalid".into(),
            ));
        }
        Ok(Self {
            radii_um: radii_um.into_boxed_slice(),
            bandwidth_um,
            integration_grid,
            simulations,
            seed,
            alpha,
            minimum_intensity_per_um2,
            limits,
        })
    }

    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }

    pub fn bandwidth_um(&self) -> f64 {
        self.bandwidth_um
    }

    pub fn integration_grid(&self) -> [usize; 2] {
        self.integration_grid
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

    pub fn minimum_intensity_per_um2(&self) -> f64 {
        self.minimum_intensity_per_um2
    }

    pub fn limits(&self) -> InhomogeneousSpatialLimits {
        self.limits
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousIntensityPoint {
    pub row: usize,
    pub intensity_per_um2: f64,
    pub boundary_mass: f64,
    pub training_point_count: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousIntensityGridPoint {
    pub probe_index: usize,
    pub x_um: f64,
    pub y_um: f64,
    pub intensity_per_um2: f64,
    pub cell_mass: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousIntensitySummary {
    pub estimator: String,
    pub kernel: String,
    pub cross_fit: String,
    pub boundary_correction: String,
    pub bandwidth_um: f64,
    pub integration_grid: [usize; 2],
    pub retained_probe_count: usize,
    pub probe_spacing_um: [f64; 2],
    pub maximum_probe_displacement_um: f64,
    pub minimum_intensity_per_um2: f64,
    pub observed_minimum_intensity_per_um2: f64,
    pub observed_maximum_intensity_per_um2: f64,
    pub artifact_digest: String,
    pub point_values: Vec<InhomogeneousIntensityPoint>,
    pub fixed_grid_digest: String,
    pub fixed_grid_total_mass: f64,
    pub fixed_grid: Vec<InhomogeneousIntensityGridPoint>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InhomogeneousSpatialPointStatus {
    Available,
    NoEligibleCenters,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousSpatialPoint {
    pub radius_um: f64,
    pub status: InhomogeneousSpatialPointStatus,
    pub eligible_centers: usize,
    pub directed_pairs: usize,
    pub inverse_intensity_pair_sum: f64,
    pub eligible_center_inverse_intensity_sum: f64,
    pub k: Option<f64>,
    pub l: Option<f64>,
    pub theoretical_k: f64,
    pub theoretical_l: f64,
    pub inference_eligible: bool,
    pub lower_l: Option<f64>,
    pub upper_l: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousSpatialInference {
    pub null_model: String,
    pub randomization_unit: String,
    pub p_global: Option<f64>,
    pub erl_depth: Option<f64>,
    pub critical_depth: Option<f64>,
    pub eligible_radius_count: usize,
    pub simulations_completed: usize,
    pub seed: u64,
    pub alpha: f64,
    pub null_draws: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousSpatialResult {
    pub case_id: String,
    pub timepoint: String,
    pub window: ClassicalWindowSummary,
    pub intensity: InhomogeneousIntensitySummary,
    pub edge_correction: String,
    pub configuration_digest: String,
    pub observed_pair_visits: usize,
    pub total_pair_visits: usize,
    pub intensity_evaluations: usize,
    pub estimated_storage_bytes: usize,
    pub limits: InhomogeneousSpatialLimits,
    pub curve: Vec<InhomogeneousSpatialPoint>,
    pub inference: InhomogeneousSpatialInference,
}

#[derive(Debug, Error)]
pub enum InhomogeneousSpatialError {
    #[error("inhomogeneous spatial resource limits must be positive")]
    InvalidResourceLimit,
    #[error("invalid inhomogeneous spatial configuration: {0}")]
    InvalidConfig(String),
    #[error("inhomogeneous spatial analysis requires at least two valid points")]
    InsufficientPoints,
    #[error("inhomogeneous intensity at row {row} is {observed}; minimum is {minimum}")]
    NearZeroIntensity {
        row: usize,
        observed: f64,
        minimum: f64,
    },
    #[error("inhomogeneous intensity evaluations exceeded maximum {maximum}")]
    IntensityEvaluationLimitExceeded { maximum: usize },
    #[error("inhomogeneous pair visits exceeded maximum {maximum}")]
    PairVisitLimitExceeded { maximum: usize },
    #[error("inhomogeneous null draws exceeded maximum {maximum}")]
    NullDrawLimitExceeded { maximum: usize },
    #[error("inhomogeneous compartment queries exceeded maximum {maximum}")]
    CompartmentQueryLimitExceeded { maximum: usize },
    #[error("point row {row} lies exactly on the declared compartment interface")]
    PointOnCompartmentInterface { row: usize },
    #[error(
        "compartment {compartment_id} has {observed} events; leave-one-out intensity requires at least two"
    )]
    SparseCompartment {
        compartment_id: String,
        observed: usize,
    },
    #[error("inhomogeneous workflow requires {required} retained bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("inhomogeneous workflow allocation failed")]
    AllocationFailed,
    #[error("inhomogeneous workflow size arithmetic overflow")]
    SizeOverflow,
    #[error("inhomogeneous dependency failed: {0}")]
    Dependency(String),
}

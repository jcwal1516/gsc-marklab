use serde::{Deserialize, Serialize};

use crate::ClassicalWindowSummary;

use super::{InhomogeneousSpatialError, InhomogeneousSpatialInference, InhomogeneousSpatialPoint};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Explicit work and retained-memory ceilings for one piecewise-compartment K/L analysis.
pub struct PiecewiseCompartmentSpatialLimits {
    /// Maximum observed points.
    pub maximum_points: usize,
    /// Maximum requested radii.
    pub maximum_radii: usize,
    /// Maximum observed point-to-compartment membership queries.
    pub maximum_compartment_queries: usize,
    /// Maximum indexed directed pair visits across observed and null patterns.
    pub maximum_pair_visits: usize,
    /// Maximum rejection-sampling candidate draws across all null patterns.
    pub maximum_null_draws: usize,
    /// Maximum conservatively estimated retained bytes.
    pub maximum_retained_bytes: usize,
}

impl PiecewiseCompartmentSpatialLimits {
    /// Construct positive resource ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_radii: usize,
        maximum_compartment_queries: usize,
        maximum_pair_visits: usize,
        maximum_null_draws: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, InhomogeneousSpatialError> {
        if [
            maximum_points,
            maximum_radii,
            maximum_compartment_queries,
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
            maximum_compartment_queries,
            maximum_pair_visits,
            maximum_null_draws,
            maximum_retained_bytes,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Configuration for exact binary-compartment piecewise intensity and border K/L.
pub struct PiecewiseCompartmentSpatialConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) limits: PiecewiseCompartmentSpatialLimits,
}

impl PiecewiseCompartmentSpatialConfig {
    /// Validate increasing positive radii and resolvable ERL inference controls.
    pub fn new(
        radii_um: Vec<f64>,
        simulations: usize,
        seed: u64,
        alpha: f64,
        limits: PiecewiseCompartmentSpatialLimits,
    ) -> Result<Self, InhomogeneousSpatialError> {
        if radii_um.is_empty()
            || radii_um.len() > limits.maximum_radii
            || radii_um.iter().any(|radius| {
                !radius.is_finite()
                    || *radius <= 0.0
                    || !(std::f64::consts::PI * radius * radius).is_finite()
            })
            || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
            || simulations == 0
            || !alpha.is_finite()
            || alpha <= 0.0
            || alpha >= 1.0
            || (simulations.saturating_add(1) as f64) * alpha < 1.0
        {
            return Err(InhomogeneousSpatialError::InvalidConfig(
                "radii or inference controls are invalid".into(),
            ));
        }
        Ok(Self {
            radii_um: radii_um.into_boxed_slice(),
            simulations,
            seed,
            alpha,
            limits,
        })
    }

    /// Requested physical radii in strictly increasing micrometres.
    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }

    /// Number of fixed-compartment-count null patterns.
    pub fn simulations(&self) -> usize {
        self.simulations
    }

    /// Deterministic base seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Simultaneous two-sided ERL level.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Bound resource controls.
    pub fn limits(&self) -> PiecewiseCompartmentSpatialLimits {
        self.limits
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Oriented role in the exact binary compartment partition.
pub enum PiecewiseCompartmentRole {
    /// Compartment with negative signed interface distance.
    Negative,
    /// Compartment with positive signed interface distance.
    Positive,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Leave-one-out intensity assigned to one observed event.
pub struct PiecewiseCompartmentIntensityPoint {
    /// Zero-based input row.
    pub row: usize,
    /// Oriented compartment role.
    pub role: PiecewiseCompartmentRole,
    /// Stable compartment identifier.
    pub compartment_id: String,
    /// Complete observed event count in this compartment.
    pub observed_compartment_count: usize,
    /// Same-compartment training count after excluding this event.
    pub training_point_count: usize,
    /// Training count divided by exact compartment area.
    pub intensity_per_um2: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Persisted piecewise intensity level for one oriented compartment.
pub struct PiecewiseCompartmentIntensityLevel {
    /// Oriented partition role.
    pub role: PiecewiseCompartmentRole,
    /// Stable compartment identifier.
    pub compartment_id: String,
    /// Exact compartment area in square micrometres.
    pub area_um2: f64,
    /// Observed event count.
    pub event_count: usize,
    /// Per-event leave-one-out training count.
    pub leave_one_out_training_count: usize,
    /// Piecewise-constant leave-one-out event intensity.
    pub intensity_per_um2: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Exact identity and observed event values for the piecewise estimator.
pub struct PiecewiseCompartmentIntensitySummary {
    /// Stable estimator name.
    pub estimator: String,
    /// Cross-fitting policy.
    pub cross_fit: String,
    /// Area normalization policy.
    pub boundary_correction: String,
    /// Policy for observed events exactly on the shared interface.
    pub interface_event_policy: String,
    /// Exact oriented partition identity.
    pub partition_digest: String,
    /// Exact persisted intensity-artifact identity.
    pub artifact_digest: String,
    /// Negative-role intensity level.
    pub negative: PiecewiseCompartmentIntensityLevel,
    /// Positive-role intensity level.
    pub positive: PiecewiseCompartmentIntensityLevel,
    /// Row-aligned observed event intensities.
    pub point_values: Vec<PiecewiseCompartmentIntensityPoint>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Typed piecewise-compartment intensity-reweighted K/L result.
pub struct PiecewiseCompartmentSpatialResult {
    /// Case identity inherited from the point pattern.
    pub case_id: String,
    /// Timepoint identity inherited from the point pattern.
    pub timepoint: String,
    /// Exact analyzed observation-window summary.
    pub window: ClassicalWindowSummary,
    /// Persisted piecewise intensity artifact.
    pub intensity: PiecewiseCompartmentIntensitySummary,
    /// Named spatial edge correction.
    pub edge_correction: String,
    /// Exact analysis-configuration identity.
    pub configuration_digest: String,
    /// Observed compartment membership queries performed.
    pub compartment_queries: usize,
    /// Pair visits used by the observed curve.
    pub observed_pair_visits: usize,
    /// Pair visits across observed and null curves.
    pub total_pair_visits: usize,
    /// Conservative retained-storage estimate.
    pub estimated_storage_bytes: usize,
    /// Bound resource controls.
    pub limits: PiecewiseCompartmentSpatialLimits,
    /// Standard-border intensity-reweighted K/L curve.
    pub curve: Vec<InhomogeneousSpatialPoint>,
    /// Whole-pattern fixed-compartment-count ERL inference.
    pub inference: InhomogeneousSpatialInference,
}

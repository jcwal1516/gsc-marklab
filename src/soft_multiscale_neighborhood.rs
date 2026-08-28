use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    classical::SpatialGeometryPlan2D, compartment_interface::analysis::measurement_status_name,
    ClassicalSpatialLimits, DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
    SoftNeighborhoodCompositionRow,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Hard resource ceilings for one prespecified multiscale soft-neighborhood analysis.
pub struct SoftMultiscaleNeighborhoodLimits {
    /// Maximum admitted cell rows.
    pub maximum_points: usize,
    /// Maximum ordered simplex classes.
    pub maximum_classes: usize,
    /// Maximum prespecified physical radii.
    pub maximum_radii: usize,
    /// Maximum contiguous simplex values across all rows.
    pub maximum_values: usize,
    /// Maximum directed neighbor visits summed across all radii.
    pub maximum_pair_visits: usize,
    /// Maximum conservatively estimated retained and peak working bytes.
    pub maximum_retained_bytes: usize,
}

impl SoftMultiscaleNeighborhoodLimits {
    /// Validate strictly positive resource ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_classes: usize,
        maximum_radii: usize,
        maximum_values: usize,
        maximum_pair_visits: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, SoftMultiscaleNeighborhoodError> {
        if [
            maximum_points,
            maximum_classes,
            maximum_radii,
            maximum_values,
            maximum_pair_visits,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(SoftMultiscaleNeighborhoodError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_classes,
            maximum_radii,
            maximum_values,
            maximum_pair_visits,
            maximum_retained_bytes,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
/// A strictly increasing physical-radius schedule and its resource policy.
pub struct SoftMultiscaleNeighborhoodConfig {
    radii_um: Box<[f64]>,
    limits: SoftMultiscaleNeighborhoodLimits,
}

impl SoftMultiscaleNeighborhoodConfig {
    /// Validate a nonempty, finite, positive, strictly increasing radius schedule.
    pub fn new(
        radii_um: Vec<f64>,
        limits: SoftMultiscaleNeighborhoodLimits,
    ) -> Result<Self, SoftMultiscaleNeighborhoodError> {
        if radii_um.is_empty()
            || radii_um.len() > limits.maximum_radii
            || radii_um.iter().any(|radius| {
                !radius.is_finite() || *radius <= 0.0 || !(radius * radius).is_finite()
            })
            || radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(SoftMultiscaleNeighborhoodError::InvalidConfig);
        }
        Ok(Self {
            radii_um: radii_um.into_boxed_slice(),
            limits,
        })
    }

    /// Return the caller-prespecified radii in micrometres.
    pub fn radii_um(&self) -> &[f64] {
        &self.radii_um
    }

    /// Return the exact resource policy bound into result identity.
    pub fn limits(&self) -> SoftMultiscaleNeighborhoodLimits {
        self.limits
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Complete per-cell and aggregate soft composition at one physical radius.
pub struct SoftMultiscaleNeighborhoodScale {
    /// Physical neighborhood radius in micrometres.
    pub radius_um: f64,
    /// Directed neighbor incidences visited at this radius.
    pub directed_pair_visits: usize,
    /// Number of focal cells with no neighbor at this radius.
    pub zero_neighbor_cell_count: usize,
    /// Directed-incidence-weighted target class mass, or `None` for an empty graph.
    pub mean_neighbor_class_mass: Option<Vec<f64>>,
    /// Row-aligned focal-cell neighborhood summaries.
    pub rows: Vec<SoftNeighborhoodCompositionRow>,
    /// Exact identity of this radius graph and its bound configuration.
    pub graph_digest: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Prespecified multiscale soft-neighborhood result for one specimen pattern.
pub struct SoftMultiscaleNeighborhoodResult {
    /// Case identity inherited from the typed pattern.
    pub case_id: String,
    /// Timepoint identity inherited from the typed pattern.
    pub timepoint: String,
    /// Probability-simplex mark identifier.
    pub mark_id: String,
    /// Declared measurement status for the simplex column.
    pub measurement_status: String,
    /// Physical coordinate-frame identifier.
    pub coordinate_frame_id: String,
    /// Exact observation-window identity.
    pub window_digest: String,
    /// Ordered simplex class codebook.
    pub class_ids: Vec<String>,
    /// Exact input, radius-list, and resource-policy identity.
    pub configuration_digest: String,
    /// Number of canonical geometry plans built for all scales; always one.
    pub geometry_build_count: usize,
    /// Directed neighbor incidences summed across all scales.
    pub total_directed_pair_visits: usize,
    /// One result per caller-supplied radius, in the same order.
    pub scales: Vec<SoftMultiscaleNeighborhoodScale>,
    /// Total-variation distance between adjacent available aggregate vectors.
    ///
    /// An entry is `None` when either adjacent radius has no neighbor incidences.
    pub adjacent_scale_total_variation_distance: Vec<Option<f64>>,
    /// Conservative retained and peak working-byte estimate.
    pub estimated_storage_bytes: usize,
    /// Exact resource ceilings applied to this execution.
    pub limits: SoftMultiscaleNeighborhoodLimits,
}

/// Compute complete-simplex neighborhood composition over prespecified physical radii.
///
/// The input MarkTable and observation window must share the same physical coordinate frame. The
/// function builds one deterministic geometry plan, performs no scale selection, and returns an
/// explicit unavailable state for scales without neighbor incidences.
pub fn soft_multiscale_neighborhood_composition(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftMultiscaleNeighborhoodConfig,
) -> Result<SoftMultiscaleNeighborhoodResult, SoftMultiscaleNeighborhoodError> {
    if window.coordinate_frame_id() != Some(input.coordinate_frame_id()) {
        return Err(SoftMultiscaleNeighborhoodError::CoordinateFrameMismatch);
    }
    let table = input
        .mark_table()
        .ok_or(SoftMultiscaleNeighborhoodError::MissingProbabilitySimplex)?;
    let values = table
        .probability_simplex_values(mark_id)
        .ok_or(SoftMultiscaleNeighborhoodError::MissingProbabilitySimplex)?;
    let levels = table
        .probability_simplex_levels(mark_id)
        .ok_or(SoftMultiscaleNeighborhoodError::MissingProbabilitySimplex)?;
    let status = table
        .measurement_status(mark_id)
        .ok_or(SoftMultiscaleNeighborhoodError::MissingProbabilitySimplex)?;
    let points = input.pattern().len();
    let classes = levels.len();
    if points == 0 {
        return Err(SoftMultiscaleNeighborhoodError::EmptyInput);
    }
    if points > config.limits.maximum_points {
        return Err(SoftMultiscaleNeighborhoodError::PointLimitExceeded);
    }
    if classes > config.limits.maximum_classes {
        return Err(SoftMultiscaleNeighborhoodError::ClassLimitExceeded);
    }
    let expected_values = points
        .checked_mul(classes)
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    if values.len() != expected_values || values.len() > config.limits.maximum_values {
        return Err(SoftMultiscaleNeighborhoodError::ValueLimitExceeded);
    }
    let geometry_limits = ClassicalSpatialLimits::new(
        config.limits.maximum_points,
        config.limits.maximum_radii,
        config.limits.maximum_pair_visits,
        1,
        config.limits.maximum_retained_bytes,
    )
    .map_err(geometry)?;
    let plan = SpatialGeometryPlan2D::new(
        &input.pattern().x_um,
        &input.pattern().y_um,
        window,
        geometry_limits,
    )
    .map_err(geometry)?;
    let scale_values = config
        .radii_um
        .len()
        .checked_mul(points)
        .and_then(|value| value.checked_mul(classes))
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    let row_storage = config
        .radii_um
        .len()
        .checked_mul(points)
        .and_then(|value| value.checked_mul(std::mem::size_of::<SoftNeighborhoodCompositionRow>()))
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    let aggregate_storage = config
        .radii_um
        .len()
        .checked_mul(classes)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    let scale_storage = config
        .radii_um
        .len()
        .checked_mul(std::mem::size_of::<SoftMultiscaleNeighborhoodScale>())
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    let adjacent_storage = config
        .radii_um
        .len()
        .saturating_sub(1)
        .checked_mul(std::mem::size_of::<Option<f64>>())
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    let scratch = points
        .checked_mul(std::mem::size_of::<crate::geom::spatial_index::Neighbor>())
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    let cell_text = input
        .cell_ids()
        .iter()
        .try_fold(0_usize, |total, cell| {
            total.checked_add(cell.as_str().len())
        })
        .and_then(|value| value.checked_mul(config.radii_um.len()))
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    let class_text = levels
        .iter()
        .try_fold(0_usize, |total, level| total.checked_add(level.len()))
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    let estimated_storage_bytes = plan
        .estimated_storage_bytes()
        .checked_add(scale_values)
        .and_then(|value| value.checked_add(row_storage))
        .and_then(|value| value.checked_add(aggregate_storage))
        .and_then(|value| value.checked_add(scale_storage))
        .and_then(|value| value.checked_add(adjacent_storage))
        .and_then(|value| value.checked_add(scratch))
        .and_then(|value| value.checked_add(cell_text))
        .and_then(|value| value.checked_add(class_text))
        .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(SoftMultiscaleNeighborhoodError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }
    let configuration_digest = configuration_digest(input, window, mark_id, config)?;
    let mut scales = Vec::new();
    scales
        .try_reserve_exact(config.radii_um.len())
        .map_err(|_| SoftMultiscaleNeighborhoodError::AllocationFailed)?;
    let mut total_visits = 0_usize;
    for radius in &config.radii_um {
        let scale = evaluate_scale(
            input,
            values,
            classes,
            &plan,
            *radius,
            configuration_digest,
            &mut total_visits,
            config.limits.maximum_pair_visits,
        )?;
        scales.push(scale);
    }
    let adjacent_scale_total_variation_distance = scales
        .windows(2)
        .map(|pair| {
            match (
                pair[0].mean_neighbor_class_mass.as_deref(),
                pair[1].mean_neighbor_class_mass.as_deref(),
            ) {
                (Some(left), Some(right)) => Some(
                    0.5 * left
                        .iter()
                        .zip(right)
                        .map(|(left, right)| (left - right).abs())
                        .sum::<f64>(),
                ),
                _ => None,
            }
        })
        .collect();
    Ok(SoftMultiscaleNeighborhoodResult {
        case_id: input.pattern().meta.case_id.clone(),
        timepoint: input.pattern().meta.timepoint.clone(),
        mark_id: mark_id.as_str().into(),
        measurement_status: measurement_status_name(status).into(),
        coordinate_frame_id: input.coordinate_frame_id().as_str().into(),
        window_digest: window.descriptor().logical_digest.to_string(),
        class_ids: levels.to_vec(),
        configuration_digest: configuration_digest.to_string(),
        geometry_build_count: 1,
        total_directed_pair_visits: total_visits,
        scales,
        adjacent_scale_total_variation_distance,
        estimated_storage_bytes,
        limits: config.limits,
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_scale(
    input: &DeclaredScalarPatternInput<'_>,
    values: &[f32],
    classes: usize,
    plan: &SpatialGeometryPlan2D,
    radius: f64,
    configuration_digest: ContentDigest,
    total_visits: &mut usize,
    maximum_visits: usize,
) -> Result<SoftMultiscaleNeighborhoodScale, SoftMultiscaleNeighborhoodError> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(input.pattern().len())
        .map_err(|_| SoftMultiscaleNeighborhoodError::AllocationFailed)?;
    let mut mass = vec![0.0; classes];
    let mut mass_correction = vec![0.0; classes];
    let mut visits = 0_usize;
    let mut zeros = 0_usize;
    for source in 0..input.pattern().len() {
        let neighbors = plan
            .index()
            .within_radius(source, radius)
            .map_err(geometry)?;
        visits = visits
            .checked_add(neighbors.len())
            .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
        *total_visits = total_visits
            .checked_add(neighbors.len())
            .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
        if *total_visits > maximum_visits {
            return Err(SoftMultiscaleNeighborhoodError::PairVisitLimitExceeded {
                observed: *total_visits,
                maximum: maximum_visits,
            });
        }
        let mean = if neighbors.is_empty() {
            zeros = zeros
                .checked_add(1)
                .ok_or(SoftMultiscaleNeighborhoodError::SizeOverflow)?;
            None
        } else {
            let mut sums = vec![0.0; classes];
            let mut corrections = vec![0.0; classes];
            for neighbor in &neighbors {
                let start = neighbor.index * classes;
                for class in 0..classes {
                    let value = f64::from(values[start + class]);
                    compensated_add(&mut sums[class], &mut corrections[class], value);
                    compensated_add(&mut mass[class], &mut mass_correction[class], value);
                }
            }
            Some(
                sums.into_iter()
                    .zip(corrections)
                    .map(|(sum, correction)| (sum + correction) / neighbors.len() as f64)
                    .collect(),
            )
        };
        rows.push(SoftNeighborhoodCompositionRow {
            row: source,
            cell_id: input.cell_ids()[source].as_str().into(),
            neighbor_count: neighbors.len(),
            mean_neighbor_probabilities: mean,
        });
    }
    let mean_neighbor_class_mass = if visits == 0 {
        None
    } else {
        Some(
            mass.into_iter()
                .zip(mass_correction)
                .map(|(sum, correction)| (sum + correction) / visits as f64)
                .collect(),
        )
    };
    let graph_digest = ContentDigest::from_framed([
        b"marklab-soft-multiscale-radius-graph-v1".as_slice(),
        plan.logical_digest().as_bytes(),
        &radius.to_bits().to_be_bytes(),
        configuration_digest.as_bytes(),
    ]);
    Ok(SoftMultiscaleNeighborhoodScale {
        radius_um: radius,
        directed_pair_visits: visits,
        zero_neighbor_cell_count: zeros,
        mean_neighbor_class_mass,
        rows,
        graph_digest: graph_digest.to_string(),
    })
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftMultiscaleNeighborhoodConfig,
) -> Result<ContentDigest, SoftMultiscaleNeighborhoodError> {
    let declared = input.declared_artifact_ref().map_err(|error| {
        SoftMultiscaleNeighborhoodError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    let mut fields = vec![
        b"marklab-soft-multiscale-neighborhood-v1".to_vec(),
        declared.digest().as_bytes().to_vec(),
        window.descriptor().logical_digest.as_bytes().to_vec(),
        mark_id.as_str().as_bytes().to_vec(),
    ];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        (config.limits.maximum_points as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_classes as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_radii as u128).to_be_bytes().to_vec(),
        (config.limits.maximum_values as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_pair_visits as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_retained_bytes as u128)
            .to_be_bytes()
            .to_vec(),
    ]);
    Ok(ContentDigest::from_framed(fields.iter().map(Vec::as_slice)))
}

fn compensated_add(sum: &mut f64, correction: &mut f64, value: f64) {
    let corrected = value - *correction;
    let next = *sum + corrected;
    *correction = (next - *sum) - corrected;
    *sum = next;
}

fn geometry(error: impl std::fmt::Display) -> SoftMultiscaleNeighborhoodError {
    SoftMultiscaleNeighborhoodError::Geometry {
        reason: error.to_string(),
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
/// Explicit validation, resource, geometry, and allocation failures for multiscale composition.
pub enum SoftMultiscaleNeighborhoodError {
    /// At least one resource ceiling was zero.
    #[error("soft multiscale neighborhood resource limits must be positive")]
    InvalidResourceLimit,
    /// The radius schedule was empty, over limit, non-finite, non-positive, or non-increasing.
    #[error("soft multiscale radii must be nonempty, bounded, finite, positive, and increasing")]
    InvalidConfig,
    /// The requested probability-simplex column was absent.
    #[error("typed probability-simplex column is missing")]
    MissingProbabilitySimplex,
    /// The admitted pattern contained no cell rows.
    #[error("soft multiscale neighborhood requires at least one row")]
    EmptyInput,
    /// The pattern and window use different coordinate frames.
    #[error("soft multiscale input and window frames differ")]
    CoordinateFrameMismatch,
    /// The point ceiling was exceeded.
    #[error("soft multiscale point limit exceeded")]
    PointLimitExceeded,
    /// The simplex-class ceiling was exceeded.
    #[error("soft multiscale class limit exceeded")]
    ClassLimitExceeded,
    /// The contiguous simplex-value ceiling or expected shape was violated.
    #[error("soft multiscale value limit exceeded")]
    ValueLimitExceeded,
    /// Total directed neighbor work exceeded its ceiling.
    #[error("soft multiscale pair visits exceeded {maximum} at {observed}")]
    PairVisitLimitExceeded { observed: usize, maximum: usize },
    /// The conservative retained-byte estimate exceeded its ceiling.
    #[error("soft multiscale requires {required} bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    /// The typed declared input failed revalidation.
    #[error("soft multiscale declared input is invalid: {reason}")]
    InvalidDeclaredInput { reason: String },
    /// Exact spatial geometry construction or traversal failed.
    #[error("soft multiscale geometry failed: {reason}")]
    Geometry { reason: String },
    /// Checked size or work arithmetic overflowed.
    #[error("soft multiscale size arithmetic overflow")]
    SizeOverflow,
    /// A bounded allocation failed.
    #[error("soft multiscale allocation failed")]
    AllocationFailed,
}

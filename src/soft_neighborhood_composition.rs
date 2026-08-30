use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::summation::kahan_add;

use crate::{
    classical::SpatialGeometryPlan2D, compartment_interface::analysis::measurement_status_name,
    ClassicalSpatialLimits, DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftNeighborhoodCompositionLimits {
    pub maximum_points: usize,
    pub maximum_classes: usize,
    pub maximum_values: usize,
    pub maximum_pair_visits: usize,
    pub maximum_retained_bytes: usize,
}

impl SoftNeighborhoodCompositionLimits {
    pub fn new(
        maximum_points: usize,
        maximum_classes: usize,
        maximum_values: usize,
        maximum_pair_visits: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, SoftNeighborhoodCompositionError> {
        if [
            maximum_points,
            maximum_classes,
            maximum_values,
            maximum_pair_visits,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(SoftNeighborhoodCompositionError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_classes,
            maximum_values,
            maximum_pair_visits,
            maximum_retained_bytes,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoftNeighborhoodCompositionConfig {
    radius_um: f64,
    limits: SoftNeighborhoodCompositionLimits,
}

impl SoftNeighborhoodCompositionConfig {
    pub fn new(
        radius_um: f64,
        limits: SoftNeighborhoodCompositionLimits,
    ) -> Result<Self, SoftNeighborhoodCompositionError> {
        if !radius_um.is_finite() || radius_um <= 0.0 || !(radius_um * radius_um).is_finite() {
            return Err(SoftNeighborhoodCompositionError::InvalidConfig);
        }
        Ok(Self { radius_um, limits })
    }

    pub fn radius_um(&self) -> f64 {
        self.radius_um
    }

    pub fn limits(&self) -> SoftNeighborhoodCompositionLimits {
        self.limits
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftNeighborhoodCompositionRow {
    pub row: usize,
    pub cell_id: String,
    pub neighbor_count: usize,
    pub mean_neighbor_probabilities: Option<Vec<f64>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftNeighborhoodCompositionResult {
    pub case_id: String,
    pub timepoint: String,
    pub mark_id: String,
    pub measurement_status: String,
    pub coordinate_frame_id: String,
    pub window_digest: String,
    pub radius_um: f64,
    pub class_ids: Vec<String>,
    pub graph_digest: String,
    pub configuration_digest: String,
    pub directed_pair_visits: usize,
    pub zero_neighbor_cell_count: usize,
    pub mean_neighbor_class_mass: Option<Vec<f64>>,
    pub rows: Vec<SoftNeighborhoodCompositionRow>,
    pub estimated_storage_bytes: usize,
    pub limits: SoftNeighborhoodCompositionLimits,
}

pub fn soft_neighborhood_composition(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftNeighborhoodCompositionConfig,
) -> Result<SoftNeighborhoodCompositionResult, SoftNeighborhoodCompositionError> {
    if window.coordinate_frame_id() != Some(input.coordinate_frame_id()) {
        return Err(SoftNeighborhoodCompositionError::CoordinateFrameMismatch);
    }
    let table = input
        .mark_table()
        .ok_or(SoftNeighborhoodCompositionError::MissingProbabilitySimplex)?;
    let values = table
        .probability_simplex_values(mark_id)
        .ok_or(SoftNeighborhoodCompositionError::MissingProbabilitySimplex)?;
    let levels = table
        .probability_simplex_levels(mark_id)
        .ok_or(SoftNeighborhoodCompositionError::MissingProbabilitySimplex)?;
    let status = table
        .measurement_status(mark_id)
        .ok_or(SoftNeighborhoodCompositionError::MissingProbabilitySimplex)?;
    let points = input.pattern().len();
    let classes = levels.len();
    if points == 0 {
        return Err(SoftNeighborhoodCompositionError::EmptyInput);
    }
    let expected_values = points
        .checked_mul(classes)
        .ok_or(SoftNeighborhoodCompositionError::SizeOverflow)?;
    if points > config.limits.maximum_points {
        return Err(SoftNeighborhoodCompositionError::PointLimitExceeded {
            observed: points,
            maximum: config.limits.maximum_points,
        });
    }
    if classes > config.limits.maximum_classes {
        return Err(SoftNeighborhoodCompositionError::ClassLimitExceeded {
            observed: classes,
            maximum: config.limits.maximum_classes,
        });
    }
    if values.len() != expected_values || values.len() > config.limits.maximum_values {
        return Err(SoftNeighborhoodCompositionError::ValueLimitExceeded {
            observed: values.len(),
            maximum: config.limits.maximum_values,
        });
    }
    let geometry_limits = ClassicalSpatialLimits::new(
        config.limits.maximum_points,
        1,
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
    let row_vectors = points
        .checked_mul(classes)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(SoftNeighborhoodCompositionError::SizeOverflow)?;
    let result_rows = points
        .checked_mul(std::mem::size_of::<SoftNeighborhoodCompositionRow>())
        .ok_or(SoftNeighborhoodCompositionError::SizeOverflow)?;
    let neighbor_scratch = points
        .checked_mul(std::mem::size_of::<crate::geom::spatial_index::Neighbor>())
        .ok_or(SoftNeighborhoodCompositionError::SizeOverflow)?;
    let cell_text = input
        .cell_ids()
        .iter()
        .try_fold(0_usize, |total, cell| {
            total.checked_add(cell.as_str().len())
        })
        .ok_or(SoftNeighborhoodCompositionError::SizeOverflow)?;
    let estimated_storage_bytes = plan
        .estimated_storage_bytes()
        .checked_add(row_vectors)
        .and_then(|value| value.checked_add(result_rows))
        .and_then(|value| value.checked_add(neighbor_scratch))
        .and_then(|value| value.checked_add(cell_text))
        .ok_or(SoftNeighborhoodCompositionError::SizeOverflow)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(
            SoftNeighborhoodCompositionError::RetainedByteLimitExceeded {
                required: estimated_storage_bytes,
                maximum: config.limits.maximum_retained_bytes,
            },
        );
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(points)
        .map_err(|_| SoftNeighborhoodCompositionError::AllocationFailed)?;
    let mut total_mass = vec![0.0; classes];
    let mut total_correction = vec![0.0; classes];
    let mut directed_pair_visits = 0_usize;
    let mut zero_neighbor_cell_count = 0_usize;
    for source in 0..points {
        let neighbors = plan
            .index()
            .within_radius(source, config.radius_um)
            .map_err(geometry)?;
        directed_pair_visits = directed_pair_visits
            .checked_add(neighbors.len())
            .ok_or(SoftNeighborhoodCompositionError::SizeOverflow)?;
        if directed_pair_visits > config.limits.maximum_pair_visits {
            return Err(SoftNeighborhoodCompositionError::PairVisitLimitExceeded {
                observed: directed_pair_visits,
                maximum: config.limits.maximum_pair_visits,
            });
        }
        let mean_neighbor_probabilities = if neighbors.is_empty() {
            zero_neighbor_cell_count = zero_neighbor_cell_count
                .checked_add(1)
                .ok_or(SoftNeighborhoodCompositionError::SizeOverflow)?;
            None
        } else {
            let mut sums = vec![0.0; classes];
            let mut corrections = vec![0.0; classes];
            for neighbor in &neighbors {
                let start = neighbor.index * classes;
                for class in 0..classes {
                    let value = f64::from(values[start + class]);
                    kahan_add(&mut sums[class], &mut corrections[class], value);
                    kahan_add(&mut total_mass[class], &mut total_correction[class], value);
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
            mean_neighbor_probabilities,
        });
    }
    let mean_neighbor_class_mass = if directed_pair_visits == 0 {
        None
    } else {
        Some(
            total_mass
                .into_iter()
                .zip(total_correction)
                .map(|(sum, correction)| (sum + correction) / directed_pair_visits as f64)
                .collect(),
        )
    };
    let configuration_digest = configuration_digest(input, window, mark_id, config)?;
    let graph_digest = ContentDigest::from_framed([
        b"marklab-soft-neighborhood-radius-graph-v1".as_slice(),
        plan.logical_digest().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        configuration_digest.as_bytes(),
    ]);
    Ok(SoftNeighborhoodCompositionResult {
        case_id: input.pattern().meta.case_id.clone(),
        timepoint: input.pattern().meta.timepoint.clone(),
        mark_id: mark_id.as_str().into(),
        measurement_status: measurement_status_name(status).into(),
        coordinate_frame_id: input.coordinate_frame_id().as_str().into(),
        window_digest: window.descriptor().logical_digest.to_string(),
        radius_um: config.radius_um,
        class_ids: levels.to_vec(),
        graph_digest: graph_digest.to_string(),
        configuration_digest: configuration_digest.to_string(),
        directed_pair_visits,
        zero_neighbor_cell_count,
        mean_neighbor_class_mass,
        rows,
        estimated_storage_bytes,
        limits: config.limits,
    })
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftNeighborhoodCompositionConfig,
) -> Result<ContentDigest, SoftNeighborhoodCompositionError> {
    let declared = input.declared_artifact_ref().map_err(|error| {
        SoftNeighborhoodCompositionError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    Ok(ContentDigest::from_framed([
        b"marklab-soft-neighborhood-composition-v1".as_slice(),
        declared.digest().as_bytes(),
        window.descriptor().logical_digest.as_bytes(),
        mark_id.as_str().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        &(config.limits.maximum_points as u128).to_be_bytes(),
        &(config.limits.maximum_classes as u128).to_be_bytes(),
        &(config.limits.maximum_values as u128).to_be_bytes(),
        &(config.limits.maximum_pair_visits as u128).to_be_bytes(),
        &(config.limits.maximum_retained_bytes as u128).to_be_bytes(),
    ]))
}

fn geometry(error: impl std::fmt::Display) -> SoftNeighborhoodCompositionError {
    SoftNeighborhoodCompositionError::Geometry {
        reason: error.to_string(),
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SoftNeighborhoodCompositionError {
    #[error("soft neighborhood resource limits must be positive")]
    InvalidResourceLimit,
    #[error("soft neighborhood radius must be finite, positive, and square to finite")]
    InvalidConfig,
    #[error("typed probability-simplex column is missing")]
    MissingProbabilitySimplex,
    #[error("soft neighborhood composition requires at least one row")]
    EmptyInput,
    #[error("soft neighborhood input and window coordinate frames differ")]
    CoordinateFrameMismatch,
    #[error("soft neighborhood has {observed} points; maximum is {maximum}")]
    PointLimitExceeded { observed: usize, maximum: usize },
    #[error("soft neighborhood has {observed} classes; maximum is {maximum}")]
    ClassLimitExceeded { observed: usize, maximum: usize },
    #[error("soft neighborhood has {observed} values; maximum is {maximum}")]
    ValueLimitExceeded { observed: usize, maximum: usize },
    #[error("soft neighborhood pair visits exceeded {maximum} at {observed}")]
    PairVisitLimitExceeded { observed: usize, maximum: usize },
    #[error("soft neighborhood requires {required} bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("soft neighborhood declared input is invalid: {reason}")]
    InvalidDeclaredInput { reason: String },
    #[error("soft neighborhood geometry failed: {reason}")]
    Geometry { reason: String },
    #[error("soft neighborhood size arithmetic overflow")]
    SizeOverflow,
    #[error("soft neighborhood allocation failed")]
    AllocationFailed,
}

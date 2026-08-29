use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    classical::SpatialGeometryPlan2D, compartment_interface::analysis::measurement_status_name,
    ClassicalSpatialLimits, DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftPairMixingLimits {
    pub maximum_points: usize,
    pub maximum_classes: usize,
    pub maximum_values: usize,
    pub maximum_pair_visits: usize,
    pub maximum_probability_products: usize,
    pub maximum_retained_bytes: usize,
}

impl SoftPairMixingLimits {
    pub fn new(
        maximum_points: usize,
        maximum_classes: usize,
        maximum_values: usize,
        maximum_pair_visits: usize,
        maximum_probability_products: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, SoftPairMixingError> {
        if [
            maximum_points,
            maximum_classes,
            maximum_values,
            maximum_pair_visits,
            maximum_probability_products,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(SoftPairMixingError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_classes,
            maximum_values,
            maximum_pair_visits,
            maximum_probability_products,
            maximum_retained_bytes,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoftPairMixingConfig {
    radius_um: f64,
    limits: SoftPairMixingLimits,
}

impl SoftPairMixingConfig {
    pub fn new(radius_um: f64, limits: SoftPairMixingLimits) -> Result<Self, SoftPairMixingError> {
        if !radius_um.is_finite() || radius_um <= 0.0 || !(radius_um * radius_um).is_finite() {
            return Err(SoftPairMixingError::InvalidConfig);
        }
        Ok(Self { radius_um, limits })
    }

    pub fn radius_um(&self) -> f64 {
        self.radius_um
    }

    pub fn limits(&self) -> SoftPairMixingLimits {
        self.limits
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftPairMixingCell {
    pub source_class_index: usize,
    pub source_class_id: String,
    pub target_class_index: usize,
    pub target_class_id: String,
    pub expected_pair_mass: f64,
    pub pair_probability: Option<f64>,
    pub random_label_probability: f64,
    pub enrichment_ratio: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftPairMixingResult {
    pub case_id: String,
    pub timepoint: String,
    pub mark_id: String,
    pub measurement_status: String,
    pub coordinate_frame_id: String,
    pub window_digest: String,
    pub radius_um: f64,
    pub class_ids: Vec<String>,
    pub normalization: String,
    pub random_label_null: String,
    pub graph_digest: String,
    pub configuration_digest: String,
    pub directed_pair_visits: usize,
    pub probability_product_evaluations: usize,
    pub matrix: Vec<SoftPairMixingCell>,
    pub estimated_storage_bytes: usize,
    pub limits: SoftPairMixingLimits,
}

pub fn soft_pair_mixing(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftPairMixingConfig,
) -> Result<SoftPairMixingResult, SoftPairMixingError> {
    if window.coordinate_frame_id() != Some(input.coordinate_frame_id()) {
        return Err(SoftPairMixingError::CoordinateFrameMismatch);
    }
    let table = input
        .mark_table()
        .ok_or(SoftPairMixingError::MissingProbabilitySimplex)?;
    let values = table
        .probability_simplex_values(mark_id)
        .ok_or(SoftPairMixingError::MissingProbabilitySimplex)?;
    let levels = table
        .probability_simplex_levels(mark_id)
        .ok_or(SoftPairMixingError::MissingProbabilitySimplex)?;
    let status = table
        .measurement_status(mark_id)
        .ok_or(SoftPairMixingError::MissingProbabilitySimplex)?;
    let points = input.pattern().len();
    let classes = levels.len();
    if points < 2 {
        return Err(SoftPairMixingError::InsufficientRows);
    }
    if points > config.limits.maximum_points {
        return Err(SoftPairMixingError::PointLimitExceeded {
            observed: points,
            maximum: config.limits.maximum_points,
        });
    }
    if classes > config.limits.maximum_classes {
        return Err(SoftPairMixingError::ClassLimitExceeded {
            observed: classes,
            maximum: config.limits.maximum_classes,
        });
    }
    let expected_values = points
        .checked_mul(classes)
        .ok_or(SoftPairMixingError::SizeOverflow)?;
    if values.len() != expected_values || values.len() > config.limits.maximum_values {
        return Err(SoftPairMixingError::ValueLimitExceeded {
            observed: values.len(),
            maximum: config.limits.maximum_values,
        });
    }
    let matrix_cells = classes
        .checked_mul(classes)
        .ok_or(SoftPairMixingError::SizeOverflow)?;
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
    let matrix_bytes = matrix_cells
        .checked_mul(4 * std::mem::size_of::<f64>() + std::mem::size_of::<SoftPairMixingCell>())
        .ok_or(SoftPairMixingError::SizeOverflow)?;
    let neighbor_bytes = points
        .checked_mul(std::mem::size_of::<crate::geom::spatial_index::Neighbor>())
        .ok_or(SoftPairMixingError::SizeOverflow)?;
    let class_text = levels
        .iter()
        .try_fold(0_usize, |total, level| total.checked_add(level.len()))
        .ok_or(SoftPairMixingError::SizeOverflow)?;
    let estimated_storage_bytes = plan
        .estimated_storage_bytes()
        .checked_add(matrix_bytes)
        .and_then(|value| value.checked_add(neighbor_bytes))
        .and_then(|value| value.checked_add(class_text))
        .ok_or(SoftPairMixingError::SizeOverflow)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(SoftPairMixingError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }

    let mut observed = vec![0.0_f64; matrix_cells];
    let mut observed_correction = vec![0.0_f64; matrix_cells];
    let mut directed_pair_visits = 0_usize;
    let mut probability_product_evaluations = 0_usize;
    for source in 0..points {
        let neighbors = plan
            .index()
            .within_radius(source, config.radius_um)
            .map_err(geometry)?;
        directed_pair_visits = directed_pair_visits
            .checked_add(neighbors.len())
            .ok_or(SoftPairMixingError::SizeOverflow)?;
        if directed_pair_visits > config.limits.maximum_pair_visits {
            return Err(SoftPairMixingError::PairVisitLimitExceeded {
                observed: directed_pair_visits,
                maximum: config.limits.maximum_pair_visits,
            });
        }
        let products = neighbors
            .len()
            .checked_mul(matrix_cells)
            .ok_or(SoftPairMixingError::SizeOverflow)?;
        probability_product_evaluations = probability_product_evaluations
            .checked_add(products)
            .ok_or(SoftPairMixingError::SizeOverflow)?;
        if probability_product_evaluations > config.limits.maximum_probability_products {
            return Err(SoftPairMixingError::ProbabilityProductLimitExceeded {
                observed: probability_product_evaluations,
                maximum: config.limits.maximum_probability_products,
            });
        }
        for neighbor in neighbors {
            let source_start = source * classes;
            let target_start = neighbor.index * classes;
            for source_class in 0..classes {
                for target_class in 0..classes {
                    let index = source_class * classes + target_class;
                    compensated_add(
                        &mut observed[index],
                        &mut observed_correction[index],
                        f64::from(values[source_start + source_class])
                            * f64::from(values[target_start + target_class]),
                    );
                }
            }
        }
    }

    let null_products = points
        .checked_mul(matrix_cells)
        .ok_or(SoftPairMixingError::SizeOverflow)?;
    probability_product_evaluations = probability_product_evaluations
        .checked_add(null_products)
        .ok_or(SoftPairMixingError::SizeOverflow)?;
    if probability_product_evaluations > config.limits.maximum_probability_products {
        return Err(SoftPairMixingError::ProbabilityProductLimitExceeded {
            observed: probability_product_evaluations,
            maximum: config.limits.maximum_probability_products,
        });
    }
    let mut class_sums = vec![0.0_f64; classes];
    let mut same_row_products = vec![0.0_f64; matrix_cells];
    for row in 0..points {
        for source_class in 0..classes {
            class_sums[source_class] += f64::from(values[row * classes + source_class]);
            for target_class in 0..classes {
                same_row_products[source_class * classes + target_class] +=
                    f64::from(values[row * classes + source_class])
                        * f64::from(values[row * classes + target_class]);
            }
        }
    }
    let random_denominator = points
        .checked_mul(points - 1)
        .ok_or(SoftPairMixingError::SizeOverflow)? as f64;
    let mut matrix = Vec::new();
    matrix
        .try_reserve_exact(matrix_cells)
        .map_err(|_| SoftPairMixingError::AllocationFailed)?;
    for source_class in 0..classes {
        for target_class in 0..classes {
            let index = source_class * classes + target_class;
            let expected_pair_mass = canonical_zero(observed[index] + observed_correction[index]);
            let pair_probability = (directed_pair_visits > 0)
                .then(|| canonical_zero(expected_pair_mass / directed_pair_visits as f64));
            let random_label_probability = canonical_zero(
                (class_sums[source_class] * class_sums[target_class] - same_row_products[index])
                    .max(0.0)
                    / random_denominator,
            );
            let enrichment_ratio = pair_probability.and_then(|probability| {
                (random_label_probability > 0.0)
                    .then(|| canonical_zero(probability / random_label_probability))
            });
            matrix.push(SoftPairMixingCell {
                source_class_index: source_class,
                source_class_id: levels[source_class].clone(),
                target_class_index: target_class,
                target_class_id: levels[target_class].clone(),
                expected_pair_mass,
                pair_probability,
                random_label_probability,
                enrichment_ratio,
            });
        }
    }
    let configuration_digest = configuration_digest(input, window, mark_id, config)?;
    let graph_digest = ContentDigest::from_framed([
        b"marklab-soft-pair-radius-graph-v1".as_slice(),
        plan.logical_digest().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
    ]);
    Ok(SoftPairMixingResult {
        case_id: input.pattern().meta.case_id.clone(),
        timepoint: input.pattern().meta.timepoint.clone(),
        mark_id: mark_id.as_str().into(),
        measurement_status: measurement_status_name(status).into(),
        coordinate_frame_id: input.coordinate_frame_id().as_str().into(),
        window_digest: window.descriptor().logical_digest.to_string(),
        radius_um: config.radius_um,
        class_ids: levels.to_vec(),
        normalization: "expected_source_target_probability_mass_per_directed_radius_pair".into(),
        random_label_null: "complete_probability_rows_without_replacement".into(),
        graph_digest: graph_digest.to_string(),
        configuration_digest: configuration_digest.to_string(),
        directed_pair_visits,
        probability_product_evaluations,
        matrix,
        estimated_storage_bytes,
        limits: config.limits,
    })
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &SoftPairMixingConfig,
) -> Result<ContentDigest, SoftPairMixingError> {
    let declared = input.declared_artifact_ref().map_err(|error| {
        SoftPairMixingError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    Ok(ContentDigest::from_framed([
        b"marklab-soft-pair-mixing-v1".as_slice(),
        declared.digest().as_bytes(),
        window.descriptor().logical_digest.as_bytes(),
        mark_id.as_str().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        &(config.limits.maximum_points as u128).to_be_bytes(),
        &(config.limits.maximum_classes as u128).to_be_bytes(),
        &(config.limits.maximum_values as u128).to_be_bytes(),
        &(config.limits.maximum_pair_visits as u128).to_be_bytes(),
        &(config.limits.maximum_probability_products as u128).to_be_bytes(),
        &(config.limits.maximum_retained_bytes as u128).to_be_bytes(),
    ]))
}

fn compensated_add(sum: &mut f64, correction: &mut f64, value: f64) {
    let corrected = value - *correction;
    let next = *sum + corrected;
    *correction = (next - *sum) - corrected;
    *sum = next;
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn geometry(error: impl std::fmt::Display) -> SoftPairMixingError {
    SoftPairMixingError::Geometry {
        reason: error.to_string(),
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SoftPairMixingError {
    #[error("soft pair-mixing resource limits must be positive")]
    InvalidResourceLimit,
    #[error("soft pair-mixing radius must be finite, positive, and square to finite")]
    InvalidConfig,
    #[error("typed probability-simplex column is missing")]
    MissingProbabilitySimplex,
    #[error("soft pair mixing requires at least two complete rows")]
    InsufficientRows,
    #[error("soft pair-mixing input and window coordinate frames differ")]
    CoordinateFrameMismatch,
    #[error("soft pair mixing has {observed} points; maximum is {maximum}")]
    PointLimitExceeded { observed: usize, maximum: usize },
    #[error("soft pair mixing has {observed} classes; maximum is {maximum}")]
    ClassLimitExceeded { observed: usize, maximum: usize },
    #[error("soft pair mixing has {observed} values; maximum is {maximum}")]
    ValueLimitExceeded { observed: usize, maximum: usize },
    #[error("soft pair-mixing visits exceeded {maximum} at {observed}")]
    PairVisitLimitExceeded { observed: usize, maximum: usize },
    #[error("soft pair-mixing probability products exceeded {maximum} at {observed}")]
    ProbabilityProductLimitExceeded { observed: usize, maximum: usize },
    #[error("soft pair mixing requires {required} bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("soft pair-mixing declared input is invalid: {reason}")]
    InvalidDeclaredInput { reason: String },
    #[error("soft pair-mixing geometry failed: {reason}")]
    Geometry { reason: String },
    #[error("soft pair-mixing size arithmetic overflow")]
    SizeOverflow,
    #[error("soft pair-mixing allocation failed")]
    AllocationFailed,
}

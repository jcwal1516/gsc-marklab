use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::summation::kahan_add;
use crate::{
    classical::SpatialGeometryPlan2D, compartment_interface::analysis::measurement_status_name,
    ClassicalSpatialLimits, DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
};

pub(crate) const TOP1_PAIR_MIXING_UNCERTAINTY_SEMANTICS: &str =
    "coordinatewise_conservative_bounds_from_top1_probability_with_declared_source_class_count_without_simplex_imputation";

/// Fixed work and retained-memory ceilings for top-1 multiclass pair bounds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Top1PairMixingBoundsLimits {
    pub maximum_points: usize,
    pub maximum_classes: usize,
    pub maximum_pair_visits: usize,
    pub maximum_bound_products: usize,
    pub maximum_retained_bytes: usize,
}

impl Top1PairMixingBoundsLimits {
    pub fn new(
        maximum_points: usize,
        maximum_classes: usize,
        maximum_pair_visits: usize,
        maximum_bound_products: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, Top1PairMixingBoundsError> {
        if [
            maximum_points,
            maximum_classes,
            maximum_pair_visits,
            maximum_bound_products,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(Top1PairMixingBoundsError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_classes,
            maximum_pair_visits,
            maximum_bound_products,
            maximum_retained_bytes,
        })
    }
}

/// One fixed physical scale for conservative top-1 probability bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct Top1PairMixingBoundsConfig {
    radius_um: f64,
    source_class_count: usize,
    limits: Top1PairMixingBoundsLimits,
}

impl Top1PairMixingBoundsConfig {
    pub fn new(
        radius_um: f64,
        source_class_count: usize,
        limits: Top1PairMixingBoundsLimits,
    ) -> Result<Self, Top1PairMixingBoundsError> {
        if !radius_um.is_finite()
            || radius_um <= 0.0
            || !(radius_um * radius_um).is_finite()
            || source_class_count < 3
            || source_class_count > limits.maximum_classes
        {
            return Err(Top1PairMixingBoundsError::InvalidConfig);
        }
        Ok(Self {
            radius_um,
            source_class_count,
            limits,
        })
    }

    pub fn radius_um(&self) -> f64 {
        self.radius_um
    }

    pub fn source_class_count(&self) -> usize {
        self.source_class_count
    }

    pub fn limits(&self) -> Top1PairMixingBoundsLimits {
        self.limits
    }
}

/// Conservative coordinatewise bound for one directed source-target class pair.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Top1PairMixingBoundCell {
    pub source_class_index: usize,
    pub source_class_id: String,
    pub target_class_index: usize,
    pub target_class_id: String,
    pub lower_expected_pair_mass: f64,
    pub upper_expected_pair_mass: f64,
    pub lower_pair_fraction: f64,
    pub upper_pair_fraction: f64,
}

/// Physical-radius multiclass pair envelope implied by top-1 labels and confidences.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Top1PairMixingBoundsResult {
    pub case_id: String,
    pub timepoint: String,
    pub categorical_mark_id: String,
    pub confidence_mark_id: String,
    pub categorical_measurement_status: String,
    pub confidence_measurement_status: String,
    pub coordinate_frame_id: String,
    pub window_digest: String,
    pub radius_um: f64,
    pub class_ids: Vec<String>,
    pub source_class_count: usize,
    pub uncertainty_semantics: String,
    pub graph_digest: String,
    pub configuration_digest: String,
    pub point_count: usize,
    pub directed_pair_visits: usize,
    pub bound_product_evaluations: usize,
    pub matrix: Vec<Top1PairMixingBoundCell>,
    pub estimated_storage_bytes: usize,
    pub limits: Top1PairMixingBoundsLimits,
}

/// Bound multiclass pair mass without fabricating unobserved class probabilities.
pub fn top1_pair_mixing_bounds(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    categorical_mark_id: &ScalarMarkId,
    confidence_mark_id: &ScalarMarkId,
    config: &Top1PairMixingBoundsConfig,
) -> Result<Top1PairMixingBoundsResult, Top1PairMixingBoundsError> {
    if window.coordinate_frame_id() != Some(input.coordinate_frame_id()) {
        return Err(Top1PairMixingBoundsError::CoordinateFrameMismatch);
    }
    let table = input
        .mark_table()
        .ok_or(Top1PairMixingBoundsError::MissingMarkTable)?;
    let winners = table
        .categorical_values(categorical_mark_id)
        .ok_or(Top1PairMixingBoundsError::MissingCategoricalMark)?;
    let levels = table
        .categorical_levels(categorical_mark_id)
        .ok_or(Top1PairMixingBoundsError::MissingCategoricalMark)?;
    let confidences = table
        .probability_values(confidence_mark_id)
        .ok_or(Top1PairMixingBoundsError::MissingConfidenceMark)?;
    let categorical_status = table
        .measurement_status(categorical_mark_id)
        .ok_or(Top1PairMixingBoundsError::MissingCategoricalMark)?;
    let confidence_status = table
        .measurement_status(confidence_mark_id)
        .ok_or(Top1PairMixingBoundsError::MissingConfidenceMark)?;
    let points = input.pattern().len();
    let classes = levels.len();
    if points < 2 {
        return Err(Top1PairMixingBoundsError::InsufficientRows);
    }
    if classes < 3 {
        return Err(Top1PairMixingBoundsError::InsufficientClasses { observed: classes });
    }
    if classes > config.source_class_count {
        return Err(Top1PairMixingBoundsError::ObservedClassesExceedSource {
            observed: classes,
            declared: config.source_class_count,
        });
    }
    if points > config.limits.maximum_points {
        return Err(Top1PairMixingBoundsError::PointLimitExceeded {
            observed: points,
            maximum: config.limits.maximum_points,
        });
    }
    if classes > config.limits.maximum_classes {
        return Err(Top1PairMixingBoundsError::ClassLimitExceeded {
            observed: classes,
            maximum: config.limits.maximum_classes,
        });
    }
    if winners.len() != points || confidences.len() != points {
        return Err(Top1PairMixingBoundsError::RowCountMismatch);
    }
    let matrix_cells = classes
        .checked_mul(classes)
        .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
    let interval_values = points
        .checked_mul(classes)
        .and_then(|value| value.checked_mul(2))
        .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
    let mut intervals = allocated_zeros(interval_values)?;
    let minimum_winner_probability = 1.0 / config.source_class_count as f64;
    for row in 0..points {
        let winner = winners[row] as usize;
        let probability = f64::from(confidences[row]);
        if winner >= classes || probability < minimum_winner_probability {
            return Err(Top1PairMixingBoundsError::InfeasibleTop1Probability {
                row,
                probability,
                minimum: minimum_winner_probability,
            });
        }
        for class in 0..classes {
            let (lower, upper) = if class == winner {
                (probability, probability)
            } else {
                (
                    (1.0 - (config.source_class_count - 1) as f64 * probability).max(0.0),
                    probability.min(1.0 - probability),
                )
            };
            let offset = 2 * (row * classes + class);
            intervals[offset] = lower;
            intervals[offset + 1] = upper;
        }
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
    let matrix_bytes = matrix_cells
        .checked_mul(
            4 * std::mem::size_of::<f64>() + std::mem::size_of::<Top1PairMixingBoundCell>(),
        )
        .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
    let interval_bytes = intervals
        .len()
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
    let neighbor_bytes = points
        .checked_mul(std::mem::size_of::<crate::geom::spatial_index::Neighbor>())
        .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
    let class_text = levels
        .iter()
        .try_fold(0_usize, |total, level| total.checked_add(level.len()))
        .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
    let estimated_storage_bytes = plan
        .estimated_storage_bytes()
        .checked_add(matrix_bytes)
        .and_then(|value| value.checked_add(interval_bytes))
        .and_then(|value| value.checked_add(neighbor_bytes))
        .and_then(|value| value.checked_add(class_text))
        .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(Top1PairMixingBoundsError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }

    let mut lower = allocated_zeros(matrix_cells)?;
    let mut lower_correction = allocated_zeros(matrix_cells)?;
    let mut upper = allocated_zeros(matrix_cells)?;
    let mut upper_correction = allocated_zeros(matrix_cells)?;
    let mut directed_pair_visits = 0_usize;
    let mut bound_product_evaluations = 0_usize;
    for source in 0..points {
        let neighbors = plan
            .index()
            .within_radius(source, config.radius_um)
            .map_err(geometry)?;
        directed_pair_visits = directed_pair_visits
            .checked_add(neighbors.len())
            .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
        if directed_pair_visits > config.limits.maximum_pair_visits {
            return Err(Top1PairMixingBoundsError::PairVisitLimitExceeded {
                observed: directed_pair_visits,
                maximum: config.limits.maximum_pair_visits,
            });
        }
        let products = neighbors
            .len()
            .checked_mul(matrix_cells)
            .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
        bound_product_evaluations = bound_product_evaluations
            .checked_add(products)
            .ok_or(Top1PairMixingBoundsError::SizeOverflow)?;
        if bound_product_evaluations > config.limits.maximum_bound_products {
            return Err(Top1PairMixingBoundsError::BoundProductLimitExceeded {
                observed: bound_product_evaluations,
                maximum: config.limits.maximum_bound_products,
            });
        }
        for neighbor in neighbors {
            for source_class in 0..classes {
                let source_offset = 2 * (source * classes + source_class);
                for target_class in 0..classes {
                    let target_offset = 2 * (neighbor.index * classes + target_class);
                    let matrix_index = source_class * classes + target_class;
                    kahan_add(
                        &mut lower[matrix_index],
                        &mut lower_correction[matrix_index],
                        intervals[source_offset] * intervals[target_offset],
                    );
                    kahan_add(
                        &mut upper[matrix_index],
                        &mut upper_correction[matrix_index],
                        intervals[source_offset + 1] * intervals[target_offset + 1],
                    );
                }
            }
        }
    }
    if directed_pair_visits == 0 {
        return Err(Top1PairMixingBoundsError::NoAdjacencyEdges);
    }
    let denominator = directed_pair_visits as f64;
    let mut matrix = Vec::new();
    matrix
        .try_reserve_exact(matrix_cells)
        .map_err(|_| Top1PairMixingBoundsError::AllocationFailed)?;
    for source_class in 0..classes {
        for target_class in 0..classes {
            let index = source_class * classes + target_class;
            matrix.push(Top1PairMixingBoundCell {
                source_class_index: source_class,
                source_class_id: levels[source_class].clone(),
                target_class_index: target_class,
                target_class_id: levels[target_class].clone(),
                lower_expected_pair_mass: lower[index],
                upper_expected_pair_mass: upper[index],
                lower_pair_fraction: lower[index] / denominator,
                upper_pair_fraction: upper[index] / denominator,
            });
        }
    }
    let configuration_digest = configuration_digest(
        input,
        window,
        categorical_mark_id,
        confidence_mark_id,
        config,
    )?;
    let graph_digest = ContentDigest::from_framed([
        b"marklab-top1-pair-mixing-radius-graph-v1".as_slice(),
        plan.logical_digest().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        configuration_digest.as_bytes(),
    ]);
    Ok(Top1PairMixingBoundsResult {
        case_id: input.pattern().meta.case_id.clone(),
        timepoint: input.pattern().meta.timepoint.clone(),
        categorical_mark_id: categorical_mark_id.as_str().into(),
        confidence_mark_id: confidence_mark_id.as_str().into(),
        categorical_measurement_status: measurement_status_name(categorical_status).into(),
        confidence_measurement_status: measurement_status_name(confidence_status).into(),
        coordinate_frame_id: input.coordinate_frame_id().as_str().into(),
        window_digest: window.descriptor().logical_digest.to_string(),
        radius_um: config.radius_um,
        class_ids: levels.to_vec(),
        source_class_count: config.source_class_count,
        uncertainty_semantics: TOP1_PAIR_MIXING_UNCERTAINTY_SEMANTICS.into(),
        graph_digest: graph_digest.to_string(),
        configuration_digest: configuration_digest.to_string(),
        point_count: points,
        directed_pair_visits,
        bound_product_evaluations,
        matrix,
        estimated_storage_bytes,
        limits: config.limits,
    })
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    categorical_mark_id: &ScalarMarkId,
    confidence_mark_id: &ScalarMarkId,
    config: &Top1PairMixingBoundsConfig,
) -> Result<ContentDigest, Top1PairMixingBoundsError> {
    let declared = input.declared_artifact_ref().map_err(|error| {
        Top1PairMixingBoundsError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    Ok(ContentDigest::from_framed([
        b"marklab-top1-pair-mixing-bounds-v1".as_slice(),
        declared.digest().as_bytes(),
        window.descriptor().logical_digest.as_bytes(),
        categorical_mark_id.as_str().as_bytes(),
        confidence_mark_id.as_str().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        &(config.source_class_count as u128).to_be_bytes(),
        &(config.limits.maximum_points as u128).to_be_bytes(),
        &(config.limits.maximum_classes as u128).to_be_bytes(),
        &(config.limits.maximum_pair_visits as u128).to_be_bytes(),
        &(config.limits.maximum_bound_products as u128).to_be_bytes(),
        &(config.limits.maximum_retained_bytes as u128).to_be_bytes(),
    ]))
}

fn allocated_zeros<T: Default + Clone>(length: usize) -> Result<Vec<T>, Top1PairMixingBoundsError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| Top1PairMixingBoundsError::AllocationFailed)?;
    values.resize(length, T::default());
    Ok(values)
}

fn geometry(error: impl std::fmt::Display) -> Top1PairMixingBoundsError {
    Top1PairMixingBoundsError::Geometry {
        reason: error.to_string(),
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum Top1PairMixingBoundsError {
    #[error("top-1 pair-mixing resource limits must be positive")]
    InvalidResourceLimit,
    #[error("top-1 pair-mixing radius and declared source class count are invalid")]
    InvalidConfig,
    #[error("typed MarkTable is missing")]
    MissingMarkTable,
    #[error("typed categorical winner mark is missing")]
    MissingCategoricalMark,
    #[error("typed winning-confidence probability mark is missing")]
    MissingConfidenceMark,
    #[error("top-1 pair mixing requires at least two rows")]
    InsufficientRows,
    #[error("top-1 pair mixing requires at least three classes; observed {observed}")]
    InsufficientClasses { observed: usize },
    #[error("observed class count {observed} exceeds declared source class count {declared}")]
    ObservedClassesExceedSource { observed: usize, declared: usize },
    #[error("point count {observed} exceeds maximum {maximum}")]
    PointLimitExceeded { observed: usize, maximum: usize },
    #[error("class count {observed} exceeds maximum {maximum}")]
    ClassLimitExceeded { observed: usize, maximum: usize },
    #[error("categorical, confidence, and point row counts differ")]
    RowCountMismatch,
    #[error(
        "row {row} winning probability {probability} is below the {minimum} minimum for a top-1 class"
    )]
    InfeasibleTop1Probability {
        row: usize,
        probability: f64,
        minimum: f64,
    },
    #[error("pair visits {observed} exceed maximum {maximum}")]
    PairVisitLimitExceeded { observed: usize, maximum: usize },
    #[error("bound products {observed} exceed maximum {maximum}")]
    BoundProductLimitExceeded { observed: usize, maximum: usize },
    #[error("retained bytes {required} exceed maximum {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("radius graph contains no adjacency edges")]
    NoAdjacencyEdges,
    #[error("coordinate frame does not match the observation window")]
    CoordinateFrameMismatch,
    #[error("declared input is invalid: {reason}")]
    InvalidDeclaredInput { reason: String },
    #[error("geometry planning failed: {reason}")]
    Geometry { reason: String },
    #[error("top-1 pair-mixing size arithmetic overflowed")]
    SizeOverflow,
    #[error("top-1 pair-mixing allocation failed")]
    AllocationFailed,
}

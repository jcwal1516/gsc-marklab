use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    classical::SpatialGeometryPlan2D, compartment_interface::analysis::measurement_status_name,
    ClassicalSpatialLimits, DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
};

/// Fixed work and retained-memory ceilings for one multiclass radius graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalNeighborhoodMixingLimits {
    pub maximum_points: usize,
    pub maximum_classes: usize,
    pub maximum_pair_visits: usize,
    pub maximum_retained_bytes: usize,
}

impl CategoricalNeighborhoodMixingLimits {
    pub fn new(
        maximum_points: usize,
        maximum_classes: usize,
        maximum_pair_visits: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, CategoricalNeighborhoodMixingError> {
        if [
            maximum_points,
            maximum_classes,
            maximum_pair_visits,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(CategoricalNeighborhoodMixingError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_classes,
            maximum_pair_visits,
            maximum_retained_bytes,
        })
    }
}

/// One fixed physical adjacency scale and its resource ceilings.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoricalNeighborhoodMixingConfig {
    radius_um: f64,
    limits: CategoricalNeighborhoodMixingLimits,
}

impl CategoricalNeighborhoodMixingConfig {
    pub fn new(
        radius_um: f64,
        limits: CategoricalNeighborhoodMixingLimits,
    ) -> Result<Self, CategoricalNeighborhoodMixingError> {
        if !radius_um.is_finite() || radius_um <= 0.0 || !(radius_um * radius_um).is_finite() {
            return Err(CategoricalNeighborhoodMixingError::InvalidConfig);
        }
        Ok(Self { radius_um, limits })
    }

    pub fn radius_um(&self) -> f64 {
        self.radius_um
    }

    pub fn limits(&self) -> CategoricalNeighborhoodMixingLimits {
        self.limits
    }
}

/// One source class's neighborhood-label distribution.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalNeighborhoodClassSummary {
    pub class_id: String,
    pub cell_count: usize,
    pub zero_neighbor_cell_count: usize,
    pub neighbor_incidences: Vec<usize>,
    pub neighbor_probabilities: Option<Vec<f64>>,
    pub neighbor_label_entropy_nats: Option<f64>,
    pub normalized_neighbor_label_entropy: Option<f64>,
}

/// Exact multiclass categorical mixing matrix on one physical-radius graph.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalNeighborhoodMixingResult {
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
    pub point_count: usize,
    pub cell_counts: Vec<usize>,
    pub undirected_edge_count: usize,
    pub directed_pair_visits: usize,
    pub directed_pair_counts: Vec<usize>,
    pub observed_pair_fractions: Vec<f64>,
    pub random_label_pair_expectations: Vec<f64>,
    pub pair_fraction_excess: Vec<f64>,
    pub cross_class_edge_count: usize,
    pub cross_class_edge_fraction: f64,
    pub random_label_cross_edge_expectation: f64,
    pub cross_edge_fraction_minus_expectation: f64,
    pub zero_neighbor_cell_count: usize,
    pub class_summaries: Vec<CategoricalNeighborhoodClassSummary>,
    pub estimated_storage_bytes: usize,
    pub limits: CategoricalNeighborhoodMixingLimits,
}

/// Compute hard-class mixing without treating cells or edges as population replicates.
pub fn categorical_neighborhood_mixing(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &CategoricalNeighborhoodMixingConfig,
) -> Result<CategoricalNeighborhoodMixingResult, CategoricalNeighborhoodMixingError> {
    if window.coordinate_frame_id() != Some(input.coordinate_frame_id()) {
        return Err(CategoricalNeighborhoodMixingError::CoordinateFrameMismatch);
    }
    let table = input
        .mark_table()
        .ok_or(CategoricalNeighborhoodMixingError::MissingCategoricalMark)?;
    let values = table
        .categorical_values(mark_id)
        .ok_or(CategoricalNeighborhoodMixingError::MissingCategoricalMark)?;
    let levels = table
        .categorical_levels(mark_id)
        .ok_or(CategoricalNeighborhoodMixingError::MissingCategoricalMark)?;
    let status = table
        .measurement_status(mark_id)
        .ok_or(CategoricalNeighborhoodMixingError::MissingCategoricalMark)?;
    let points = input.pattern().len();
    let classes = levels.len();
    if points == 0 {
        return Err(CategoricalNeighborhoodMixingError::EmptyInput);
    }
    if classes < 3 {
        return Err(CategoricalNeighborhoodMixingError::InsufficientClasses { observed: classes });
    }
    if points > config.limits.maximum_points {
        return Err(CategoricalNeighborhoodMixingError::PointLimitExceeded {
            observed: points,
            maximum: config.limits.maximum_points,
        });
    }
    if classes > config.limits.maximum_classes {
        return Err(CategoricalNeighborhoodMixingError::ClassLimitExceeded {
            observed: classes,
            maximum: config.limits.maximum_classes,
        });
    }
    if values.len() != points {
        return Err(CategoricalNeighborhoodMixingError::RowCountMismatch {
            points,
            values: values.len(),
        });
    }
    let matrix_len = classes
        .checked_mul(classes)
        .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
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
    let matrix_bytes = matrix_len
        .checked_mul(std::mem::size_of::<usize>() + 4 * std::mem::size_of::<f64>())
        .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
    let summary_bytes = classes
        .checked_mul(std::mem::size_of::<CategoricalNeighborhoodClassSummary>())
        .and_then(|value| {
            value.checked_add(
                matrix_len
                    .checked_mul(std::mem::size_of::<usize>() + std::mem::size_of::<f64>())?,
            )
        })
        .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
    let neighbor_scratch = points
        .checked_mul(std::mem::size_of::<crate::geom::spatial_index::Neighbor>())
        .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
    let text_bytes = input
        .cell_ids()
        .iter()
        .try_fold(0_usize, |total, cell| {
            total.checked_add(cell.as_str().len())
        })
        .and_then(|total| {
            levels
                .iter()
                .try_fold(total, |total, level| total.checked_add(level.len()))
        })
        .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
    let estimated_storage_bytes = plan
        .estimated_storage_bytes()
        .checked_add(matrix_bytes)
        .and_then(|value| value.checked_add(summary_bytes))
        .and_then(|value| value.checked_add(neighbor_scratch))
        .and_then(|value| value.checked_add(text_bytes))
        .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(
            CategoricalNeighborhoodMixingError::RetainedByteLimitExceeded {
                required: estimated_storage_bytes,
                maximum: config.limits.maximum_retained_bytes,
            },
        );
    }

    let mut cell_counts: Vec<usize> = allocated_zeros(classes)?;
    for code in values {
        cell_counts[*code as usize] += 1;
    }
    let mut directed_pair_counts: Vec<usize> = allocated_zeros(matrix_len)?;
    let mut zero_neighbor_by_class: Vec<usize> = allocated_zeros(classes)?;
    let mut directed_pair_visits = 0_usize;
    for source in 0..points {
        let neighbors = plan
            .index()
            .within_radius(source, config.radius_um)
            .map_err(geometry)?;
        directed_pair_visits = directed_pair_visits
            .checked_add(neighbors.len())
            .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
        if directed_pair_visits > config.limits.maximum_pair_visits {
            return Err(CategoricalNeighborhoodMixingError::PairVisitLimitExceeded {
                observed: directed_pair_visits,
                maximum: config.limits.maximum_pair_visits,
            });
        }
        let source_class = values[source] as usize;
        if neighbors.is_empty() {
            zero_neighbor_by_class[source_class] += 1;
        }
        for neighbor in neighbors {
            let index = source_class * classes + values[neighbor.index] as usize;
            directed_pair_counts[index] = directed_pair_counts[index]
                .checked_add(1)
                .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
        }
    }
    if directed_pair_visits == 0 {
        return Err(CategoricalNeighborhoodMixingError::NoAdjacencyEdges);
    }
    if !directed_pair_visits.is_multiple_of(2) {
        return Err(CategoricalNeighborhoodMixingError::AsymmetricAdjacency);
    }
    let undirected_edge_count = directed_pair_visits / 2;
    let denominator = directed_pair_visits as f64;
    let population_denominator = points
        .checked_mul(points - 1)
        .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?
        as f64;
    let mut observed_pair_fractions: Vec<f64> = allocated_zeros(matrix_len)?;
    let mut random_label_pair_expectations: Vec<f64> = allocated_zeros(matrix_len)?;
    let mut pair_fraction_excess: Vec<f64> = allocated_zeros(matrix_len)?;
    let mut cross_directed = 0_usize;
    let mut random_same = 0.0_f64;
    for source_class in 0..classes {
        for target_class in 0..classes {
            let index = source_class * classes + target_class;
            observed_pair_fractions[index] = directed_pair_counts[index] as f64 / denominator;
            let numerator = if source_class == target_class {
                cell_counts[source_class].checked_mul(cell_counts[source_class].saturating_sub(1))
            } else {
                cell_counts[source_class].checked_mul(cell_counts[target_class])
            }
            .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
            random_label_pair_expectations[index] = numerator as f64 / population_denominator;
            pair_fraction_excess[index] =
                observed_pair_fractions[index] - random_label_pair_expectations[index];
            if source_class == target_class {
                random_same += random_label_pair_expectations[index];
            } else {
                cross_directed = cross_directed
                    .checked_add(directed_pair_counts[index])
                    .ok_or(CategoricalNeighborhoodMixingError::SizeOverflow)?;
            }
        }
    }
    if !cross_directed.is_multiple_of(2) {
        return Err(CategoricalNeighborhoodMixingError::AsymmetricAdjacency);
    }
    let cross_class_edge_count = cross_directed / 2;
    let cross_class_edge_fraction = cross_class_edge_count as f64 / undirected_edge_count as f64;
    let random_label_cross_edge_expectation = 1.0 - random_same;
    let class_summaries = (0..classes)
        .map(|class| {
            let incidences = directed_pair_counts[class * classes..(class + 1) * classes].to_vec();
            let total = incidences.iter().sum::<usize>();
            let probabilities = (total > 0).then(|| {
                incidences
                    .iter()
                    .map(|count| *count as f64 / total as f64)
                    .collect::<Vec<_>>()
            });
            let entropy = probabilities.as_ref().map(|values| entropy_nats(values));
            CategoricalNeighborhoodClassSummary {
                class_id: levels[class].clone(),
                cell_count: cell_counts[class],
                zero_neighbor_cell_count: zero_neighbor_by_class[class],
                neighbor_incidences: incidences,
                neighbor_probabilities: probabilities,
                neighbor_label_entropy_nats: entropy,
                normalized_neighbor_label_entropy: entropy
                    .map(|value| value / (classes as f64).ln()),
            }
        })
        .collect::<Vec<_>>();
    let configuration_digest = configuration_digest(input, window, mark_id, config)?;
    let graph_digest = ContentDigest::from_framed([
        b"marklab-categorical-neighborhood-radius-graph-v1".as_slice(),
        plan.logical_digest().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        configuration_digest.as_bytes(),
    ]);
    Ok(CategoricalNeighborhoodMixingResult {
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
        point_count: points,
        cell_counts,
        undirected_edge_count,
        directed_pair_visits,
        directed_pair_counts,
        observed_pair_fractions,
        random_label_pair_expectations,
        pair_fraction_excess,
        cross_class_edge_count,
        cross_class_edge_fraction,
        random_label_cross_edge_expectation,
        cross_edge_fraction_minus_expectation: cross_class_edge_fraction
            - random_label_cross_edge_expectation,
        zero_neighbor_cell_count: zero_neighbor_by_class.iter().sum(),
        class_summaries,
        estimated_storage_bytes,
        limits: config.limits,
    })
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &CategoricalNeighborhoodMixingConfig,
) -> Result<ContentDigest, CategoricalNeighborhoodMixingError> {
    let declared = input.declared_artifact_ref().map_err(|error| {
        CategoricalNeighborhoodMixingError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    Ok(ContentDigest::from_framed([
        b"marklab-categorical-neighborhood-mixing-v1".as_slice(),
        declared.digest().as_bytes(),
        window.descriptor().logical_digest.as_bytes(),
        mark_id.as_str().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        &(config.limits.maximum_points as u128).to_be_bytes(),
        &(config.limits.maximum_classes as u128).to_be_bytes(),
        &(config.limits.maximum_pair_visits as u128).to_be_bytes(),
        &(config.limits.maximum_retained_bytes as u128).to_be_bytes(),
    ]))
}

pub(crate) fn entropy_nats(probabilities: &[f64]) -> f64 {
    let value = probabilities
        .iter()
        .filter(|probability| **probability > 0.0)
        .map(|probability| -probability * probability.ln())
        .sum::<f64>();
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn allocated_zeros<T: Default + Clone>(
    length: usize,
) -> Result<Vec<T>, CategoricalNeighborhoodMixingError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| CategoricalNeighborhoodMixingError::AllocationFailed)?;
    values.resize(length, T::default());
    Ok(values)
}

fn geometry(error: impl std::fmt::Display) -> CategoricalNeighborhoodMixingError {
    CategoricalNeighborhoodMixingError::Geometry {
        reason: error.to_string(),
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CategoricalNeighborhoodMixingError {
    #[error("categorical neighborhood resource limits must be positive")]
    InvalidResourceLimit,
    #[error("categorical neighborhood radius must be finite, positive, and square to finite")]
    InvalidConfig,
    #[error("typed categorical mark is missing")]
    MissingCategoricalMark,
    #[error("categorical neighborhood requires at least one row")]
    EmptyInput,
    #[error(
        "categorical neighborhood requires at least three declared classes; observed {observed}"
    )]
    InsufficientClasses { observed: usize },
    #[error("categorical neighborhood input and window coordinate frames differ")]
    CoordinateFrameMismatch,
    #[error("categorical neighborhood has {observed} points; maximum is {maximum}")]
    PointLimitExceeded { observed: usize, maximum: usize },
    #[error("categorical neighborhood has {observed} classes; maximum is {maximum}")]
    ClassLimitExceeded { observed: usize, maximum: usize },
    #[error("categorical neighborhood has {points} points but {values} categorical values")]
    RowCountMismatch { points: usize, values: usize },
    #[error("categorical neighborhood graph has no adjacency edges")]
    NoAdjacencyEdges,
    #[error("categorical neighborhood graph is not exactly symmetric")]
    AsymmetricAdjacency,
    #[error("categorical neighborhood pair visits exceeded {maximum} at {observed}")]
    PairVisitLimitExceeded { observed: usize, maximum: usize },
    #[error("categorical neighborhood requires {required} bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("categorical neighborhood size arithmetic overflow")]
    SizeOverflow,
    #[error("categorical neighborhood allocation failed")]
    AllocationFailed,
    #[error("categorical neighborhood declared input is invalid: {reason}")]
    InvalidDeclaredInput { reason: String },
    #[error("categorical neighborhood geometry failed: {reason}")]
    Geometry { reason: String },
}

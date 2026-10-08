use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::summation::kahan_add;
use crate::{
    classical::SpatialGeometryPlan2D, compartment_interface::analysis::measurement_status_name,
    ClassicalSpatialLimits, DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
};

pub(crate) const CATEGORICAL_SUPPORT_SEMANTICS: &str =
    "winner_type_pixel_support_sensitivity_weighting_conditional_on_fixed_hard_labels_not_class_posterior";

/// Fixed work and retained-memory ceilings for support-weighted multiclass mixing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalSupportMixingLimits {
    pub maximum_points: usize,
    pub maximum_classes: usize,
    pub maximum_pair_visits: usize,
    pub maximum_retained_bytes: usize,
}

impl CategoricalSupportMixingLimits {
    pub fn new(
        maximum_points: usize,
        maximum_classes: usize,
        maximum_pair_visits: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, CategoricalSupportMixingError> {
        if [
            maximum_points,
            maximum_classes,
            maximum_pair_visits,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(CategoricalSupportMixingError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_classes,
            maximum_pair_visits,
            maximum_retained_bytes,
        })
    }
}

/// One fixed physical radius and its support-weighted mixing ceilings.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoricalSupportMixingConfig {
    radius_um: f64,
    limits: CategoricalSupportMixingLimits,
}

impl CategoricalSupportMixingConfig {
    pub fn new(
        radius_um: f64,
        limits: CategoricalSupportMixingLimits,
    ) -> Result<Self, CategoricalSupportMixingError> {
        if !radius_um.is_finite() || radius_um <= 0.0 || !(radius_um * radius_um).is_finite() {
            return Err(CategoricalSupportMixingError::InvalidConfig);
        }
        Ok(Self { radius_um, limits })
    }

    pub fn radius_um(&self) -> f64 {
        self.radius_um
    }

    pub fn limits(&self) -> CategoricalSupportMixingLimits {
        self.limits
    }
}

/// Hard-label pair count and its continuous support-weighted sensitivity mass.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalSupportMixingCell {
    pub source_class_index: usize,
    pub source_class_id: String,
    pub target_class_index: usize,
    pub target_class_id: String,
    pub hard_directed_pair_count: usize,
    pub hard_directed_pair_fraction: f64,
    pub support_weighted_pair_mass: f64,
    pub support_weighted_pair_fraction: f64,
    pub mean_pair_support: Option<f64>,
}

/// Exact hard-label geometry with threshold-free winner-type support sensitivity weights.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalSupportMixingResult {
    pub case_id: String,
    pub timepoint: String,
    pub categorical_mark_id: String,
    pub support_mark_id: String,
    pub categorical_measurement_status: String,
    pub support_measurement_status: String,
    pub coordinate_frame_id: String,
    pub window_digest: String,
    pub radius_um: f64,
    pub class_ids: Vec<String>,
    pub support_semantics: String,
    pub graph_digest: String,
    pub configuration_digest: String,
    pub point_count: usize,
    pub directed_pair_visits: usize,
    pub total_support_weighted_pair_mass: f64,
    pub total_support_retention_fraction: f64,
    pub matrix: Vec<CategoricalSupportMixingCell>,
    pub estimated_storage_bytes: usize,
    pub limits: CategoricalSupportMixingLimits,
}

/// Reweight fixed hard-class adjacency by source and target winner-type pixel support.
pub fn categorical_support_mixing(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    categorical_mark_id: &ScalarMarkId,
    support_mark_id: &ScalarMarkId,
    config: &CategoricalSupportMixingConfig,
) -> Result<CategoricalSupportMixingResult, CategoricalSupportMixingError> {
    if window.coordinate_frame_id() != Some(input.coordinate_frame_id()) {
        return Err(CategoricalSupportMixingError::CoordinateFrameMismatch);
    }
    let table = input
        .mark_table()
        .ok_or(CategoricalSupportMixingError::MissingMarkTable)?;
    let classes_by_row = table
        .categorical_values(categorical_mark_id)
        .ok_or(CategoricalSupportMixingError::MissingCategoricalMark)?;
    let levels = table
        .categorical_levels(categorical_mark_id)
        .ok_or(CategoricalSupportMixingError::MissingCategoricalMark)?;
    let supports = table
        .probability_values(support_mark_id)
        .ok_or(CategoricalSupportMixingError::MissingSupportMark)?;
    let categorical_status = table
        .measurement_status(categorical_mark_id)
        .ok_or(CategoricalSupportMixingError::MissingCategoricalMark)?;
    let support_status = table
        .measurement_status(support_mark_id)
        .ok_or(CategoricalSupportMixingError::MissingSupportMark)?;
    let points = input.pattern().len();
    let classes = levels.len();
    if points < 2 {
        return Err(CategoricalSupportMixingError::InsufficientRows);
    }
    if classes < 3 {
        return Err(CategoricalSupportMixingError::InsufficientClasses { observed: classes });
    }
    if points > config.limits.maximum_points {
        return Err(CategoricalSupportMixingError::PointLimitExceeded {
            observed: points,
            maximum: config.limits.maximum_points,
        });
    }
    if classes > config.limits.maximum_classes {
        return Err(CategoricalSupportMixingError::ClassLimitExceeded {
            observed: classes,
            maximum: config.limits.maximum_classes,
        });
    }
    if classes_by_row.len() != points || supports.len() != points {
        return Err(CategoricalSupportMixingError::RowCountMismatch);
    }
    let matrix_cells = classes
        .checked_mul(classes)
        .ok_or(CategoricalSupportMixingError::SizeOverflow)?;
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
            std::mem::size_of::<usize>()
                + 4 * std::mem::size_of::<f64>()
                + std::mem::size_of::<CategoricalSupportMixingCell>(),
        )
        .ok_or(CategoricalSupportMixingError::SizeOverflow)?;
    let neighbor_bytes = points
        .checked_mul(std::mem::size_of::<crate::geom::spatial_index::Neighbor>())
        .ok_or(CategoricalSupportMixingError::SizeOverflow)?;
    let class_text_bytes = levels
        .iter()
        .try_fold(0_usize, |total, level| total.checked_add(level.len()))
        .ok_or(CategoricalSupportMixingError::SizeOverflow)?;
    let estimated_storage_bytes = plan
        .estimated_storage_bytes()
        .checked_add(matrix_bytes)
        .and_then(|value| value.checked_add(neighbor_bytes))
        .and_then(|value| value.checked_add(class_text_bytes))
        .ok_or(CategoricalSupportMixingError::SizeOverflow)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(CategoricalSupportMixingError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }

    let mut hard_counts: Vec<usize> = allocated_zeros(matrix_cells)?;
    let mut support_mass: Vec<f64> = allocated_zeros(matrix_cells)?;
    let mut support_correction: Vec<f64> = allocated_zeros(matrix_cells)?;
    let mut directed_pair_visits = 0_usize;
    for source in 0..points {
        let neighbors = plan
            .index()
            .within_radius(source, config.radius_um)
            .map_err(geometry)?;
        directed_pair_visits = directed_pair_visits
            .checked_add(neighbors.len())
            .ok_or(CategoricalSupportMixingError::SizeOverflow)?;
        if directed_pair_visits > config.limits.maximum_pair_visits {
            return Err(CategoricalSupportMixingError::PairVisitLimitExceeded {
                observed: directed_pair_visits,
                maximum: config.limits.maximum_pair_visits,
            });
        }
        let source_class = classes_by_row[source] as usize;
        for neighbor in neighbors {
            let target_class = classes_by_row[neighbor.index] as usize;
            let index = source_class * classes + target_class;
            hard_counts[index] = hard_counts[index]
                .checked_add(1)
                .ok_or(CategoricalSupportMixingError::SizeOverflow)?;
            kahan_add(
                &mut support_mass[index],
                &mut support_correction[index],
                f64::from(supports[source]) * f64::from(supports[neighbor.index]),
            );
        }
    }
    if directed_pair_visits == 0 {
        return Err(CategoricalSupportMixingError::NoAdjacencyEdges);
    }
    let denominator = directed_pair_visits as f64;
    let mut matrix = Vec::new();
    matrix
        .try_reserve_exact(matrix_cells)
        .map_err(|_| CategoricalSupportMixingError::AllocationFailed)?;
    let mut total_support_weighted_pair_mass = 0.0_f64;
    let mut total_support_correction = 0.0_f64;
    for source_class in 0..classes {
        for target_class in 0..classes {
            let index = source_class * classes + target_class;
            let count = hard_counts[index];
            let mass = support_mass[index];
            kahan_add(
                &mut total_support_weighted_pair_mass,
                &mut total_support_correction,
                mass,
            );
            matrix.push(CategoricalSupportMixingCell {
                source_class_index: source_class,
                source_class_id: levels[source_class].clone(),
                target_class_index: target_class,
                target_class_id: levels[target_class].clone(),
                hard_directed_pair_count: count,
                hard_directed_pair_fraction: count as f64 / denominator,
                support_weighted_pair_mass: mass,
                support_weighted_pair_fraction: mass / denominator,
                mean_pair_support: (count > 0).then_some(mass / count as f64),
            });
        }
    }
    let configuration_digest =
        configuration_digest(input, window, categorical_mark_id, support_mark_id, config)?;
    let graph_digest = ContentDigest::from_framed([
        b"marklab-categorical-support-radius-graph-v1".as_slice(),
        plan.logical_digest().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        configuration_digest.as_bytes(),
    ]);
    Ok(CategoricalSupportMixingResult {
        case_id: input.pattern().meta.case_id.clone(),
        timepoint: input.pattern().meta.timepoint.clone(),
        categorical_mark_id: categorical_mark_id.as_str().into(),
        support_mark_id: support_mark_id.as_str().into(),
        categorical_measurement_status: measurement_status_name(categorical_status).into(),
        support_measurement_status: measurement_status_name(support_status).into(),
        coordinate_frame_id: input.coordinate_frame_id().as_str().into(),
        window_digest: window.descriptor().logical_digest.to_string(),
        radius_um: config.radius_um,
        class_ids: levels.to_vec(),
        support_semantics: CATEGORICAL_SUPPORT_SEMANTICS.into(),
        graph_digest: graph_digest.to_string(),
        configuration_digest: configuration_digest.to_string(),
        point_count: points,
        directed_pair_visits,
        total_support_weighted_pair_mass,
        total_support_retention_fraction: total_support_weighted_pair_mass / denominator,
        matrix,
        estimated_storage_bytes,
        limits: config.limits,
    })
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    categorical_mark_id: &ScalarMarkId,
    support_mark_id: &ScalarMarkId,
    config: &CategoricalSupportMixingConfig,
) -> Result<ContentDigest, CategoricalSupportMixingError> {
    let declared = input.declared_artifact_ref().map_err(|error| {
        CategoricalSupportMixingError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    Ok(ContentDigest::from_framed([
        b"marklab-categorical-support-mixing-v1".as_slice(),
        declared.digest().as_bytes(),
        window.descriptor().logical_digest.as_bytes(),
        categorical_mark_id.as_str().as_bytes(),
        support_mark_id.as_str().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        &(config.limits.maximum_points as u128).to_be_bytes(),
        &(config.limits.maximum_classes as u128).to_be_bytes(),
        &(config.limits.maximum_pair_visits as u128).to_be_bytes(),
        &(config.limits.maximum_retained_bytes as u128).to_be_bytes(),
    ]))
}

fn allocated_zeros<T: Default + Clone>(
    length: usize,
) -> Result<Vec<T>, CategoricalSupportMixingError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| CategoricalSupportMixingError::AllocationFailed)?;
    values.resize(length, T::default());
    Ok(values)
}

fn geometry(error: impl std::fmt::Display) -> CategoricalSupportMixingError {
    CategoricalSupportMixingError::Geometry {
        reason: error.to_string(),
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CategoricalSupportMixingError {
    #[error("categorical support-mixing resource limits must be positive")]
    InvalidResourceLimit,
    #[error("categorical support-mixing radius must be finite and positive")]
    InvalidConfig,
    #[error("typed MarkTable is missing")]
    MissingMarkTable,
    #[error("typed categorical mark is missing")]
    MissingCategoricalMark,
    #[error("typed winner-type support probability mark is missing")]
    MissingSupportMark,
    #[error("categorical support mixing requires at least two rows")]
    InsufficientRows,
    #[error("categorical support mixing requires at least three classes; observed {observed}")]
    InsufficientClasses { observed: usize },
    #[error("point count {observed} exceeds maximum {maximum}")]
    PointLimitExceeded { observed: usize, maximum: usize },
    #[error("class count {observed} exceeds maximum {maximum}")]
    ClassLimitExceeded { observed: usize, maximum: usize },
    #[error("categorical, support, and point row counts differ")]
    RowCountMismatch,
    #[error("pair visits {observed} exceed maximum {maximum}")]
    PairVisitLimitExceeded { observed: usize, maximum: usize },
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
    #[error("categorical support-mixing size arithmetic overflowed")]
    SizeOverflow,
    #[error("categorical support-mixing allocation failed")]
    AllocationFailed,
}

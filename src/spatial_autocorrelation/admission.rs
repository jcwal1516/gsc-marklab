use std::collections::{BTreeMap, BTreeSet};

use marklab_cohort::InferenceDesign;
use marklab_data::{CoordinateFrameId, MeasurementStatus};

use crate::{
    geom::window::ObservationWindow2D,
    scalar_mark::{DeclaredScalarPatternInput, ScalarMarkId},
};

use super::{
    weights::RadiusWeights, GlobalMoranConditioning, GlobalMoranDesign, GlobalMoranError,
    GlobalMoranLimits, GlobalMoranWeightPolicy,
};

pub(super) struct PreparedSpatialAutocorrelation {
    pub(super) frame: CoordinateFrameId,
    pub(super) measurement_status: MeasurementStatus,
    pub(super) row_count: usize,
    pub(super) values: Vec<f64>,
    pub(super) weights: RadiusWeights,
    pub(super) inference_design: InferenceDesign,
    pub(super) conditioning_mark_id: Option<ScalarMarkId>,
    pub(super) conditioning_measurement_status: Option<MeasurementStatus>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    radius_um: f64,
    weight_policy: GlobalMoranWeightPolicy,
    design: &GlobalMoranDesign,
    limits: GlobalMoranLimits,
) -> Result<PreparedSpatialAutocorrelation, GlobalMoranError> {
    if !radius_um.is_finite() || radius_um <= 0.0 {
        return Err(GlobalMoranError::InvalidRadius);
    }
    let frame = window
        .coordinate_frame_id()
        .ok_or(GlobalMoranError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id() {
        return Err(GlobalMoranError::CoordinateFrameMismatch {
            expected: input.coordinate_frame_id().clone(),
            observed: frame.clone(),
        });
    }
    let pattern = input.pattern();
    let row_count = pattern.len();
    if row_count < 3 {
        return Err(GlobalMoranError::InsufficientPoints {
            observed: row_count,
        });
    }
    if row_count > limits.maximum_points {
        return Err(GlobalMoranError::PointLimitExceeded {
            observed: row_count,
            maximum: limits.maximum_points,
        });
    }
    for (row, (&x, &y)) in pattern.x_um.iter().zip(&pattern.y_um).enumerate() {
        if !window.contains(x, y) {
            return Err(GlobalMoranError::PointOutsideWindow { row });
        }
    }
    reject_duplicate_points(&pattern.x_um, &pattern.y_um)?;

    let table = input
        .mark_table()
        .ok_or(GlobalMoranError::TypedMarkTableRequired)?;
    let values = table.continuous_values(mark_id).ok_or_else(|| {
        GlobalMoranError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    if values.len() != row_count {
        return Err(GlobalMoranError::RowCountMismatch {
            expected: row_count,
            observed: values.len(),
        });
    }
    let measurement_status = table.measurement_status(mark_id).ok_or_else(|| {
        GlobalMoranError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    let values = values
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let weights = RadiusWeights::build(
        &pattern.x_um,
        &pattern.y_um,
        radius_um,
        weight_policy,
        limits.maximum_directed_edges,
    )?;
    let work = weights
        .edges
        .len()
        .checked_mul(design.permutations)
        .ok_or(GlobalMoranError::SizeOverflow)?;
    if work > limits.maximum_permutation_edge_evaluations {
        return Err(GlobalMoranError::PermutationWorkExceeded {
            observed: work,
            maximum: limits.maximum_permutation_edge_evaluations,
        });
    }
    let (compartments, conditioning_mark_id, conditioning_measurement_status) =
        match design.conditioning {
            GlobalMoranConditioning::None => (None, None, None),
            GlobalMoranConditioning::HistologicCompartment => {
                let compartment_id = ScalarMarkId::new("histologic_compartment")
                    .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))?;
                let compartments = table
                    .categorical_values(&compartment_id)
                    .ok_or(GlobalMoranError::MissingCompartmentStratum)?;
                let status = table
                    .measurement_status(&compartment_id)
                    .ok_or(GlobalMoranError::MissingCompartmentStratum)?;
                (Some(compartments), Some(compartment_id), Some(status))
            }
        };
    let inference_design = compile_inference_design(compartments, design, &values)?;
    Ok(PreparedSpatialAutocorrelation {
        frame: frame.clone(),
        measurement_status,
        row_count,
        values,
        weights,
        inference_design,
        conditioning_mark_id,
        conditioning_measurement_status,
    })
}

fn compile_inference_design(
    compartments: Option<&[u32]>,
    design: &GlobalMoranDesign,
    values: &[f64],
) -> Result<InferenceDesign, GlobalMoranError> {
    match design.conditioning {
        GlobalMoranConditioning::None => {
            if values
                .iter()
                .map(|value| value.to_bits())
                .collect::<BTreeSet<_>>()
                .len()
                < 2
            {
                return Err(GlobalMoranError::DegenerateNull);
            }
            InferenceDesign::random_labeling(
                values.len(),
                design.permutations,
                design.seed,
                design.alternative,
            )
            .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))
        }
        GlobalMoranConditioning::HistologicCompartment => {
            let compartments = compartments.ok_or(GlobalMoranError::MissingCompartmentStratum)?;
            if compartments.len() != values.len() {
                return Err(GlobalMoranError::RowCountMismatch {
                    expected: values.len(),
                    observed: compartments.len(),
                });
            }
            let mut by_compartment = BTreeMap::<u32, Vec<usize>>::new();
            for (row, compartment) in compartments.iter().copied().enumerate() {
                by_compartment.entry(compartment).or_default().push(row);
            }
            if by_compartment.len() < 2 {
                return Err(GlobalMoranError::DegenerateNull);
            }
            let has_exchangeable_values = by_compartment.values().any(|block| {
                block.len() > 1
                    && block
                        .iter()
                        .map(|row| values[*row].to_bits())
                        .collect::<BTreeSet<_>>()
                        .len()
                        > 1
            });
            if !has_exchangeable_values {
                return Err(GlobalMoranError::DegenerateNull);
            }
            InferenceDesign::stratified_random_labeling(
                compartments,
                design.permutations,
                design.seed,
                design.alternative,
            )
            .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))
        }
    }
}

pub(super) fn reject_duplicate_points(x: &[f64], y: &[f64]) -> Result<(), GlobalMoranError> {
    let mut rows = BTreeMap::new();
    for (row, (&x, &y)) in x.iter().zip(y).enumerate() {
        let key = (canonical_bits(x), canonical_bits(y));
        if let Some(first_row) = rows.insert(key, row) {
            return Err(GlobalMoranError::DuplicatePoint {
                first_row,
                second_row: row,
            });
        }
    }
    Ok(())
}

fn canonical_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}

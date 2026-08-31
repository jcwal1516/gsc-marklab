use marklab_cohort::{InferenceAlternative, InferenceDesign};
use marklab_numerics::extreme_rank_length_envelope;

use super::{
    evaluation::evaluate_curve,
    validation::{
        has_distinct_values, has_stratified_distinct_values, pair_plan_digest, validate_bins,
        validate_points,
    },
    ScalarVariogramBin, ScalarVariogramConditioning, ScalarVariogramEnvelopeRow,
    ScalarVariogramError, ScalarVariogramInferenceDesign, ScalarVariogramInferenceLimits,
    ScalarVariogramInferenceResult, ScalarVariogramLimits, ScalarVariogramResult,
};
use crate::{
    geom::window::ObservationWindow2D,
    scalar_mark::{DeclaredScalarPatternInput, ScalarMarkId},
};

/// Compute an observed scalar semivariogram in exact caller-declared physical lag bins.
pub fn scalar_semivariogram(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    bins: &[ScalarVariogramBin],
    limits: ScalarVariogramLimits,
) -> Result<ScalarVariogramResult, ScalarVariogramError> {
    validate_bins(bins)?;
    let frame = window
        .coordinate_frame_id()
        .ok_or(ScalarVariogramError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id() {
        return Err(ScalarVariogramError::CoordinateFrameMismatch {
            expected: input.coordinate_frame_id().clone(),
            observed: frame.clone(),
        });
    }
    let pattern = input.pattern();
    let point_count = pattern.len();
    if point_count < 2 {
        return Err(ScalarVariogramError::InsufficientPoints);
    }
    if point_count > limits.maximum_points {
        return Err(ScalarVariogramError::PointLimitExceeded {
            observed: point_count,
            maximum: limits.maximum_points,
        });
    }
    let pair_visits = point_count
        .checked_mul(point_count - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(ScalarVariogramError::SizeOverflow)?;
    if pair_visits > limits.maximum_pair_visits {
        return Err(ScalarVariogramError::PairVisitLimitExceeded {
            observed: pair_visits,
            maximum: limits.maximum_pair_visits,
        });
    }
    validate_points(pattern.x_um.as_ref(), pattern.y_um.as_ref(), window)?;
    let table = input
        .mark_table()
        .ok_or(ScalarVariogramError::TypedMarkTableRequired)?;
    let values = table.continuous_values(mark_id).ok_or_else(|| {
        ScalarVariogramError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    if values.len() != point_count {
        return Err(ScalarVariogramError::RowCountMismatch {
            expected: point_count,
            observed: values.len(),
        });
    }
    let measurement_status = table.measurement_status(mark_id).ok_or_else(|| {
        ScalarVariogramError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    let declared_input_digest = input
        .declared_artifact_ref()
        .map_err(|error| ScalarVariogramError::InvalidInput(error.to_string()))?
        .digest();

    let values = values
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let curve = evaluate_curve(&pattern.x_um, &pattern.y_um, &values, bins)?;

    Ok(ScalarVariogramResult {
        mark_id: mark_id.clone(),
        measurement_status,
        coordinate_frame_id: frame.clone(),
        declared_input_digest,
        pair_plan_digest: pair_plan_digest(input, window, mark_id, declared_input_digest, bins),
        point_count,
        pair_visits,
        curve,
        edge_correction: "none_fixed_observed_locations",
        inference_status: "observed_only_no_null",
    })
}

/// Compute a deterministic blocked whole-value ERL envelope for the scalar semivariogram.
pub fn scalar_semivariogram_permutation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    bins: &[ScalarVariogramBin],
    design: &ScalarVariogramInferenceDesign,
    limits: ScalarVariogramInferenceLimits,
) -> Result<ScalarVariogramInferenceResult, ScalarVariogramError> {
    let observed = scalar_semivariogram(input, window, mark_id, bins, limits.observed)?;
    let work = observed
        .pair_visits
        .checked_mul(design.permutations)
        .ok_or(ScalarVariogramError::SizeOverflow)?;
    if work > limits.maximum_permutation_pair_evaluations {
        return Err(ScalarVariogramError::PermutationWorkExceeded {
            observed: work,
            maximum: limits.maximum_permutation_pair_evaluations,
        });
    }
    let table = input
        .mark_table()
        .ok_or(ScalarVariogramError::TypedMarkTableRequired)?;
    let values = table
        .continuous_values(mark_id)
        .ok_or_else(|| ScalarVariogramError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        })?
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let (inference_design, conditioning_mark_id, conditioning_measurement_status) =
        match design.conditioning {
            ScalarVariogramConditioning::None => {
                if !has_distinct_values(&values) {
                    return Err(ScalarVariogramError::DegenerateNull);
                }
                (
                    InferenceDesign::random_labeling(
                        values.len(),
                        design.permutations,
                        design.seed,
                        InferenceAlternative::TwoSided,
                    )
                    .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?,
                    None,
                    None,
                )
            }
            ScalarVariogramConditioning::HistologicCompartment => {
                let compartment_id = ScalarMarkId::new("histologic_compartment")
                    .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?;
                let compartments = table
                    .categorical_values(&compartment_id)
                    .ok_or(ScalarVariogramError::MissingCompartmentStratum)?;
                if compartments.len() != values.len() {
                    return Err(ScalarVariogramError::RowCountMismatch {
                        expected: values.len(),
                        observed: compartments.len(),
                    });
                }
                if !has_stratified_distinct_values(&values, compartments) {
                    return Err(ScalarVariogramError::DegenerateNull);
                }
                let status = table
                    .measurement_status(&compartment_id)
                    .ok_or(ScalarVariogramError::MissingCompartmentStratum)?;
                (
                    InferenceDesign::stratified_random_labeling(
                        compartments,
                        design.permutations,
                        design.seed,
                        InferenceAlternative::TwoSided,
                    )
                    .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?,
                    Some(compartment_id),
                    Some(status),
                )
            }
        };
    let eligible = observed
        .curve
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.semivariance.map(|value| (index, value)))
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return Err(ScalarVariogramError::NoEligibleBins);
    }
    let observed_eligible = eligible.iter().map(|(_, value)| *value).collect::<Vec<_>>();
    let pattern = input.pattern();
    let mut null_curves = Vec::with_capacity(design.permutations);
    for replicate in 0..design.permutations {
        let indices = inference_design
            .permuted_indices(replicate)
            .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?;
        let permuted = indices
            .iter()
            .map(|source| values[*source])
            .collect::<Vec<_>>();
        let curve = evaluate_curve(&pattern.x_um, &pattern.y_um, &permuted, bins)?;
        null_curves.push(
            eligible
                .iter()
                .map(|(index, _)| {
                    curve[*index]
                        .semivariance
                        .ok_or(ScalarVariogramError::NoEligibleBins)
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    let envelope = extreme_rank_length_envelope(&observed_eligible, &null_curves, design.alpha)
        .map_err(|error| ScalarVariogramError::Inference(error.to_string()))?;
    let mut eligible_position = vec![None; bins.len()];
    for (position, (index, _)) in eligible.iter().enumerate() {
        eligible_position[*index] = Some(position);
    }
    let curve = observed
        .curve
        .iter()
        .enumerate()
        .map(|(index, row)| ScalarVariogramEnvelopeRow {
            observed: row.clone(),
            lower_global_envelope: eligible_position[index]
                .map(|position| envelope.lower[position]),
            upper_global_envelope: eligible_position[index]
                .map(|position| envelope.upper[position]),
        })
        .collect();

    Ok(ScalarVariogramInferenceResult {
        observed,
        curve,
        p_global: envelope.p_global,
        observed_erl_depth: envelope.observed_depth,
        critical_erl_depth: envelope.critical_depth,
        alpha: design.alpha,
        eligible_bin_count: eligible.len(),
        stratum_count: inference_design.block_count(),
        conditioning_mark_id,
        conditioning_measurement_status,
        conditioning: design.conditioning,
        permutations_requested: design.permutations,
        permutations_completed: design.permutations,
        seed: design.seed,
        multiplicity_policy: "two_sided_extreme_rank_length_global_envelope",
    })
}

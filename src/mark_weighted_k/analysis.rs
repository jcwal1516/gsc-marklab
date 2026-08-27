use marklab_cohort::{InferenceAlternative, InferenceDesign};
use marklab_workflow::ContentDigest;

use crate::{
    classical::window_summary,
    continuous_mark_correlation::{compensated_add, mark_statistics},
    mark_pair_plan::{build_mark_pair_plan, erl_workspace_bytes, MarkPairPlan, MarkPairPlanError},
    permutation::envelopes::GlobalEnvelope,
    DeclaredScalarPatternInput, ObservationWindow2D,
};

use super::types::*;

struct Evaluation {
    points: Vec<MarkWeightedKPoint>,
    weighted_values: Vec<f64>,
    eligible: Vec<bool>,
}

/// Estimate cumulative product-weighted K with an explicit unweighted K baseline.
pub fn continuous_mark_weighted_k(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &MarkWeightedKConfig,
) -> Result<MarkWeightedKResult, MarkWeightedKError> {
    let frame = window
        .coordinate_frame_id()
        .ok_or(MarkWeightedKError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id() {
        return Err(MarkWeightedKError::CoordinateFrameMismatch);
    }
    let table = input
        .mark_table()
        .ok_or(MarkWeightedKError::MissingContinuousMark)?;
    let marks = table
        .continuous_values(&config.mark_id)
        .ok_or(MarkWeightedKError::MissingContinuousMark)?;
    let measurement_status = table
        .measurement_status(&config.mark_id)
        .ok_or(MarkWeightedKError::MissingContinuousMark)?;
    if marks.len() > config.limits.maximum_points {
        return Err(MarkWeightedKError::PointLimitExceeded {
            observed: marks.len(),
            maximum: config.limits.maximum_points,
        });
    }
    let (global_mean, global_variance, expected_weight) =
        mark_statistics(marks).map_err(mark_statistics_error)?;
    let normalization = global_mean * global_mean;
    let plan = build_mark_pair_plan(
        input,
        window,
        &config.radii_um,
        config.limits.maximum_points,
        config.limits.maximum_radii,
        config.limits.maximum_directed_pairs,
        config.limits.maximum_retained_bytes,
    )
    .map_err(pair_plan_error)?;
    let required_evaluations = plan
        .pairs
        .len()
        .checked_mul(config.permutations)
        .ok_or(MarkWeightedKError::SizeOverflow)?;
    if required_evaluations > config.limits.maximum_permutation_pair_evaluations {
        return Err(MarkWeightedKError::PermutationPairLimitExceeded {
            required: required_evaluations,
            maximum: config.limits.maximum_permutation_pair_evaluations,
        });
    }
    let matrix_value_bytes = config
        .permutations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(MarkWeightedKError::SizeOverflow)?;
    let matrix_descriptor_bytes = config
        .permutations
        .checked_mul(std::mem::size_of::<Vec<f64>>())
        .ok_or(MarkWeightedKError::SizeOverflow)?;
    let radius_work_bytes = config
        .radii_um
        .len()
        .checked_mul(
            2 * std::mem::size_of::<MarkWeightedKPoint>()
                + 8 * std::mem::size_of::<f64>()
                + 10 * std::mem::size_of::<usize>()
                + 2 * std::mem::size_of::<bool>(),
        )
        .ok_or(MarkWeightedKError::SizeOverflow)?;
    let permutation_work_bytes = marks
        .len()
        .checked_mul(
            4 * std::mem::size_of::<usize>()
                + std::mem::size_of::<f32>()
                + std::mem::size_of::<bool>(),
        )
        .ok_or(MarkWeightedKError::SizeOverflow)?;
    let erl_bytes = erl_workspace_bytes(config.permutations, config.radii_um.len())
        .ok_or(MarkWeightedKError::SizeOverflow)?;
    let retained_bytes = plan
        .retained_bytes
        .checked_add(matrix_value_bytes)
        .and_then(|value| value.checked_add(matrix_descriptor_bytes))
        .and_then(|value| value.checked_add(radius_work_bytes))
        .and_then(|value| value.checked_add(permutation_work_bytes))
        .and_then(|value| value.checked_add(erl_bytes))
        .ok_or(MarkWeightedKError::SizeOverflow)?;
    if retained_bytes > config.limits.maximum_retained_bytes {
        return Err(MarkWeightedKError::RetainedByteLimitExceeded {
            required: retained_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }

    let mut observed = evaluate(
        marks,
        normalization,
        expected_weight,
        window.area_um2(),
        &plan,
        &config.radii_um,
    )?;
    let design = InferenceDesign::random_labeling(
        marks.len(),
        config.permutations,
        config.seed,
        InferenceAlternative::TwoSided,
    )
    .map_err(dependency)?;
    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(config.permutations)
        .map_err(|_| MarkWeightedKError::AllocationFailed)?;
    for replicate in 0..config.permutations {
        let indices = design.permuted_indices(replicate).map_err(dependency)?;
        let mut permuted = Vec::new();
        permuted
            .try_reserve_exact(indices.len())
            .map_err(|_| MarkWeightedKError::AllocationFailed)?;
        permuted.extend(indices.iter().map(|index| marks[*index]));
        simulated.push(
            evaluate(
                &permuted,
                normalization,
                expected_weight,
                window.area_um2(),
                &plan,
                &config.radii_um,
            )?
            .weighted_values,
        );
    }
    let weighted_k = attach_envelope(
        &mut observed.points,
        &observed.weighted_values,
        &simulated,
        &observed.eligible,
        config.alpha,
    )?;
    Ok(MarkWeightedKResult {
        mark_id: config.mark_id.clone(),
        measurement_status,
        unit: "square_micrometer".into(),
        coordinate_frame_id: frame.clone(),
        window: window_summary(window.descriptor()),
        weight_function: "product_over_global_arithmetic_mean_squared".into(),
        edge_correction: "standard_border_reduced_sample".into(),
        global_mark_mean: global_mean,
        global_mark_population_variance: global_variance,
        expected_random_label_weight: expected_weight,
        geometry: MarkWeightedKGeometrySummary {
            point_count: marks.len(),
            directed_pair_count: plan.pairs.len(),
            geometry_build_count: 1,
            geometry_digest: plan.geometry.logical_digest(),
            pair_plan_digest: plan.digest,
            estimated_storage_bytes: retained_bytes,
        },
        configuration_digest: configuration_digest(config),
        limits: config.limits,
        curve: observed.points,
        inference: MarkWeightedKInferenceSummary {
            null_model: "random_labeling".into(),
            permutation_unit: "complete_continuous_mark_row".into(),
            alternative: "two_sided".into(),
            multiplicity: "single_weighted_k_erl_family".into(),
            weighted_k,
            permutations_requested: config.permutations,
            permutations_attempted: config.permutations,
            permutations_completed: config.permutations,
            seed: config.seed,
            alpha: config.alpha,
            permutation_pair_evaluations: required_evaluations,
        },
    })
}

fn evaluate(
    marks: &[f32],
    normalization: f64,
    expected_weight: f64,
    area_um2: f64,
    plan: &MarkPairPlan,
    radii_um: &[f64],
) -> Result<Evaluation, MarkWeightedKError> {
    let width = radii_um
        .len()
        .checked_add(1)
        .ok_or(MarkWeightedKError::SizeOverflow)?;
    let mut eligible_ends = vec![0_usize; width];
    let mut pair_starts = vec![0_usize; width];
    let mut pair_ends = vec![0_usize; width];
    let mut product_starts = vec![0.0_f64; width];
    let mut product_start_corrections = vec![0.0_f64; width];
    let mut product_ends = vec![0.0_f64; width];
    let mut product_end_corrections = vec![0.0_f64; width];
    for boundary in plan.geometry.boundary_distances() {
        let end = radii_um.partition_point(|radius| *radius <= *boundary);
        eligible_ends[end] = eligible_ends[end]
            .checked_add(1)
            .ok_or(MarkWeightedKError::SizeOverflow)?;
    }
    for pair in &plan.pairs {
        let start = radii_um.partition_point(|radius| *radius < pair.distance_um);
        let end = radii_um
            .partition_point(|radius| *radius <= plan.geometry.boundary_distances()[pair.source]);
        if start >= end || start >= radii_um.len() {
            continue;
        }
        pair_starts[start] = pair_starts[start]
            .checked_add(1)
            .ok_or(MarkWeightedKError::SizeOverflow)?;
        pair_ends[end] = pair_ends[end]
            .checked_add(1)
            .ok_or(MarkWeightedKError::SizeOverflow)?;
        let product = f64::from(marks[pair.source]) * f64::from(marks[pair.target]);
        compensated_add(
            &mut product_starts[start],
            &mut product_start_corrections[start],
            product,
        );
        compensated_add(
            &mut product_ends[end],
            &mut product_end_corrections[end],
            product,
        );
    }

    let mut eligible_centers = marks.len();
    let mut active_pairs = 0_usize;
    let mut active_product_sum = 0.0;
    let mut active_product_correction = 0.0;
    let mut points = Vec::new();
    let mut weighted_values = Vec::new();
    let mut eligible = Vec::new();
    points
        .try_reserve_exact(radii_um.len())
        .and_then(|()| weighted_values.try_reserve_exact(radii_um.len()))
        .and_then(|()| eligible.try_reserve_exact(radii_um.len()))
        .map_err(|_| MarkWeightedKError::AllocationFailed)?;
    for (index, radius_um) in radii_um.iter().copied().enumerate() {
        eligible_centers = eligible_centers
            .checked_sub(eligible_ends[index])
            .ok_or(MarkWeightedKError::SizeOverflow)?;
        active_pairs = active_pairs
            .checked_sub(pair_ends[index])
            .and_then(|value| value.checked_add(pair_starts[index]))
            .ok_or(MarkWeightedKError::SizeOverflow)?;
        compensated_add(
            &mut active_product_sum,
            &mut active_product_correction,
            -(product_ends[index] + product_end_corrections[index]),
        );
        compensated_add(
            &mut active_product_sum,
            &mut active_product_correction,
            product_starts[index] + product_start_corrections[index],
        );
        let product_sum = if active_pairs == 0 {
            0.0
        } else {
            active_product_sum + active_product_correction
        };
        let normalized_weight_sum = product_sum / normalization;
        let denominator = marks
            .len()
            .checked_mul(eligible_centers)
            .ok_or(MarkWeightedKError::SizeOverflow)?;
        let (weighted_k, unweighted_k, expected_random) = if denominator == 0 {
            (None, None, None)
        } else {
            let unweighted = area_um2 * active_pairs as f64 / denominator as f64;
            (
                Some(area_um2 * normalized_weight_sum / denominator as f64),
                Some(unweighted),
                Some(unweighted * expected_weight),
            )
        };
        weighted_values.push(weighted_k.unwrap_or(0.0));
        eligible.push(weighted_k.is_some());
        points.push(MarkWeightedKPoint {
            radius_um,
            status: if weighted_k.is_some() {
                MarkWeightedKPointStatus::Available
            } else {
                MarkWeightedKPointStatus::NoEligibleCenters
            },
            eligible_centers,
            directed_pairs: active_pairs,
            mark_product_sum: product_sum,
            normalized_weight_sum,
            weighted_k,
            unweighted_k,
            expected_random_label_weighted_k: expected_random,
            theoretical_k: std::f64::consts::PI * radius_um * radius_um,
            inference_eligible: false,
            lower_weighted_k: None,
            upper_weighted_k: None,
        });
    }
    Ok(Evaluation {
        points,
        weighted_values,
        eligible,
    })
}

fn attach_envelope(
    points: &mut [MarkWeightedKPoint],
    observed: &[f64],
    simulations: &[Vec<f64>],
    eligible: &[bool],
    alpha: f64,
) -> Result<Option<MarkWeightedKComponentInference>, MarkWeightedKError> {
    if !eligible.iter().any(|value| *value) {
        return Ok(None);
    }
    let envelope =
        GlobalEnvelope::from_curves_with_eligibility(observed, simulations, alpha, eligible)
            .map_err(dependency)?;
    for (index, point) in points.iter_mut().enumerate() {
        if eligible[index] {
            point.inference_eligible = true;
            point.lower_weighted_k = Some(envelope.lower[index]);
            point.upper_weighted_k = Some(envelope.upper[index]);
        }
    }
    Ok(Some(MarkWeightedKComponentInference {
        p_global: envelope.p_global,
        erl_depth: envelope.erl_depth,
        critical_depth: envelope.critical_depth,
        eligible_radius_count: eligible.iter().filter(|value| **value).count(),
    }))
}

pub(crate) fn configuration_digest(config: &MarkWeightedKConfig) -> ContentDigest {
    let mut fields = vec![
        b"marklab-continuous-mark-weighted-k-config-v1".to_vec(),
        config.mark_id.as_str().as_bytes().to_vec(),
    ];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        b"product_over_global_arithmetic_mean_squared".to_vec(),
        b"standard_border_reduced_sample".to_vec(),
        (config.permutations as u128).to_be_bytes().to_vec(),
        config.seed.to_be_bytes().to_vec(),
        config.alpha.to_bits().to_be_bytes().to_vec(),
        (config.limits.maximum_points as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_radii as u128).to_be_bytes().to_vec(),
        (config.limits.maximum_directed_pairs as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_permutation_pair_evaluations as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_retained_bytes as u128)
            .to_be_bytes()
            .to_vec(),
    ]);
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

fn mark_statistics_error(error: crate::ContinuousMarkCorrelationError) -> MarkWeightedKError {
    match error {
        crate::ContinuousMarkCorrelationError::ZeroMarkVariance => {
            MarkWeightedKError::ZeroMarkVariance
        }
        crate::ContinuousMarkCorrelationError::SizeOverflow => MarkWeightedKError::SizeOverflow,
        other => MarkWeightedKError::Dependency {
            reason: other.to_string(),
        },
    }
}

fn pair_plan_error(error: MarkPairPlanError) -> MarkWeightedKError {
    match error {
        MarkPairPlanError::DirectedPairLimitExceeded { maximum } => {
            MarkWeightedKError::DirectedPairLimitExceeded { maximum }
        }
        MarkPairPlanError::RetainedByteLimitExceeded { required, maximum } => {
            MarkWeightedKError::RetainedByteLimitExceeded { required, maximum }
        }
        MarkPairPlanError::AllocationFailed => MarkWeightedKError::AllocationFailed,
        MarkPairPlanError::SizeOverflow => MarkWeightedKError::SizeOverflow,
        MarkPairPlanError::Dependency(reason) => MarkWeightedKError::Dependency { reason },
    }
}

fn dependency(error: impl std::fmt::Display) -> MarkWeightedKError {
    MarkWeightedKError::Dependency {
        reason: error.to_string(),
    }
}

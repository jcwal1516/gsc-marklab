use marklab_cohort::{InferenceAlternative, InferenceDesign};
use marklab_workflow::ContentDigest;

use crate::common::finite::canonical_zero;

use crate::{
    classical::window_summary,
    mark_pair_plan::{build_mark_pair_plan, erl_workspace_bytes, MarkPairPlan, MarkPairPlanError},
    permutation::envelopes::GlobalEnvelope,
    DeclaredScalarPatternInput, ObservationWindow2D,
};

use super::types::*;

struct Evaluation {
    points: Vec<ContinuousMarkCorrelationPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
}

/// Estimate global-mean normalized continuous mark correlation with random labeling.
pub fn continuous_mark_correlation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &ContinuousMarkCorrelationConfig,
) -> Result<ContinuousMarkCorrelationResult, ContinuousMarkCorrelationError> {
    let frame = window
        .coordinate_frame_id()
        .ok_or(ContinuousMarkCorrelationError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id() {
        return Err(ContinuousMarkCorrelationError::CoordinateFrameMismatch);
    }
    let table = input
        .mark_table()
        .ok_or(ContinuousMarkCorrelationError::MissingContinuousMark)?;
    let marks = table
        .continuous_values(&config.mark_id)
        .ok_or(ContinuousMarkCorrelationError::MissingContinuousMark)?;
    let measurement_status = table
        .measurement_status(&config.mark_id)
        .ok_or(ContinuousMarkCorrelationError::MissingContinuousMark)?;
    if marks.len() > config.limits.maximum_points {
        return Err(ContinuousMarkCorrelationError::PointLimitExceeded {
            observed: marks.len(),
            maximum: config.limits.maximum_points,
        });
    }
    let (global_mean, global_variance, expected_random) = mark_statistics(marks)?;
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
        .ok_or(ContinuousMarkCorrelationError::SizeOverflow)?;
    if required_evaluations > config.limits.maximum_permutation_pair_evaluations {
        return Err(
            ContinuousMarkCorrelationError::PermutationPairLimitExceeded {
                required: required_evaluations,
                maximum: config.limits.maximum_permutation_pair_evaluations,
            },
        );
    }
    let matrix_value_bytes = config
        .permutations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(ContinuousMarkCorrelationError::SizeOverflow)?;
    let matrix_descriptor_bytes = config
        .permutations
        .checked_mul(std::mem::size_of::<Vec<f64>>())
        .ok_or(ContinuousMarkCorrelationError::SizeOverflow)?;
    let radius_work_bytes = config
        .radii_um
        .len()
        .checked_mul(
            2 * std::mem::size_of::<ContinuousMarkCorrelationPoint>()
                + 5 * std::mem::size_of::<f64>()
                + 2 * std::mem::size_of::<usize>()
                + 2 * std::mem::size_of::<bool>(),
        )
        .ok_or(ContinuousMarkCorrelationError::SizeOverflow)?;
    let permutation_work_bytes = marks
        .len()
        .checked_mul(
            4 * std::mem::size_of::<usize>()
                + std::mem::size_of::<f32>()
                + std::mem::size_of::<bool>(),
        )
        .ok_or(ContinuousMarkCorrelationError::SizeOverflow)?;
    let erl_bytes = erl_workspace_bytes(config.permutations, config.radii_um.len())
        .ok_or(ContinuousMarkCorrelationError::SizeOverflow)?;
    let retained_bytes = plan
        .retained_bytes
        .checked_add(matrix_value_bytes)
        .and_then(|value| value.checked_add(matrix_descriptor_bytes))
        .and_then(|value| value.checked_add(radius_work_bytes))
        .and_then(|value| value.checked_add(permutation_work_bytes))
        .and_then(|value| value.checked_add(erl_bytes))
        .ok_or(ContinuousMarkCorrelationError::SizeOverflow)?;
    if retained_bytes > config.limits.maximum_retained_bytes {
        return Err(ContinuousMarkCorrelationError::RetainedByteLimitExceeded {
            required: retained_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }

    let mut observed = evaluate(marks, normalization, &plan, &config.radii_um)?;
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
        .map_err(|_| ContinuousMarkCorrelationError::AllocationFailed)?;
    for replicate in 0..config.permutations {
        let indices = design.permuted_indices(replicate).map_err(dependency)?;
        let mut permuted = Vec::new();
        permuted
            .try_reserve_exact(indices.len())
            .map_err(|_| ContinuousMarkCorrelationError::AllocationFailed)?;
        permuted.extend(indices.iter().map(|index| marks[*index]));
        simulated.push(evaluate(&permuted, normalization, &plan, &config.radii_um)?.values);
    }
    let correlation = attach_envelope(
        &mut observed.points,
        &observed.values,
        &simulated,
        &observed.eligible,
        config.alpha,
    )?;
    Ok(ContinuousMarkCorrelationResult {
        mark_id: config.mark_id.clone(),
        measurement_status,
        unit: "square_micrometer".into(),
        coordinate_frame_id: frame.clone(),
        window: window_summary(window.descriptor()),
        normalization: "global_arithmetic_mean_product".into(),
        global_mark_mean: canonical_zero(global_mean),
        global_mark_population_variance: canonical_zero(global_variance),
        expected_random_label_correlation: canonical_zero(expected_random),
        geometry: ContinuousMarkCorrelationGeometrySummary {
            point_count: marks.len(),
            directed_pair_count: plan.pairs.len(),
            geometry_build_count: 1,
            geometry_digest: plan.geometry.logical_digest(),
            pair_plan_digest: plan.digest,
            edge_correction: "standard_border_reduced_sample".into(),
            estimated_storage_bytes: retained_bytes,
        },
        configuration_digest: configuration_digest(config),
        limits: config.limits,
        curve: observed.points,
        inference: ContinuousMarkCorrelationInferenceSummary {
            null_model: "random_labeling".into(),
            permutation_unit: "complete_continuous_mark_row".into(),
            alternative: "two_sided".into(),
            multiplicity: "single_correlation_erl_family".into(),
            correlation,
            permutations_requested: config.permutations,
            permutations_attempted: config.permutations,
            permutations_completed: config.permutations,
            seed: config.seed,
            alpha: config.alpha,
            permutation_pair_evaluations: required_evaluations,
        },
    })
}

pub(crate) fn mark_statistics(
    marks: &[f32],
) -> Result<(f64, f64, f64), ContinuousMarkCorrelationError> {
    if marks.len() < 2 {
        return Err(ContinuousMarkCorrelationError::ZeroMarkVariance);
    }
    let sum = compensated_sum(marks.iter().map(|value| f64::from(*value)));
    let mean = sum / marks.len() as f64;
    let variance = compensated_sum(marks.iter().map(|value| {
        let centered = f64::from(*value) - mean;
        centered * centered
    })) / marks.len() as f64;
    if variance == 0.0 {
        return Err(ContinuousMarkCorrelationError::ZeroMarkVariance);
    }
    // Accumulate the algebraically equivalent ordered mass without subtracting two large,
    // nearly equal quantities when one positive mark dominates all others.
    let mut prefix = 0.0;
    let mut prefix_correction = 0.0;
    let mut unordered_product_mass = 0.0;
    let mut product_correction = 0.0;
    for mark in marks {
        let value = f64::from(*mark);
        compensated_add(
            &mut unordered_product_mass,
            &mut product_correction,
            value * (prefix + prefix_correction),
        );
        compensated_add(&mut prefix, &mut prefix_correction, value);
    }
    let ordered_product_mass = 2.0 * (unordered_product_mass + product_correction);
    let ordered_pair_count = marks
        .len()
        .checked_mul(marks.len() - 1)
        .ok_or(ContinuousMarkCorrelationError::SizeOverflow)?;
    let expectation = ordered_product_mass / ordered_pair_count as f64 / (mean * mean);
    Ok((mean, variance, expectation))
}

fn evaluate(
    marks: &[f32],
    normalization: f64,
    plan: &MarkPairPlan,
    radii_um: &[f64],
) -> Result<Evaluation, ContinuousMarkCorrelationError> {
    let mut shell_counts = vec![0_usize; radii_um.len()];
    let mut shell_sums = vec![0.0_f64; radii_um.len()];
    let mut shell_corrections = vec![0.0_f64; radii_um.len()];
    for pair in &plan.pairs {
        let shell = radii_um.partition_point(|radius| *radius < pair.distance_um);
        let eligible_end = radii_um
            .partition_point(|radius| *radius <= plan.geometry.boundary_distances()[pair.source]);
        if shell >= eligible_end || shell >= radii_um.len() {
            continue;
        }
        shell_counts[shell] = shell_counts[shell]
            .checked_add(1)
            .ok_or(ContinuousMarkCorrelationError::SizeOverflow)?;
        compensated_add(
            &mut shell_sums[shell],
            &mut shell_corrections[shell],
            f64::from(marks[pair.source]) * f64::from(marks[pair.target]),
        );
    }
    let mut points = Vec::new();
    let mut values = Vec::new();
    let mut eligible = Vec::new();
    points
        .try_reserve_exact(radii_um.len())
        .and_then(|()| values.try_reserve_exact(radii_um.len()))
        .and_then(|()| eligible.try_reserve_exact(radii_um.len()))
        .map_err(|_| ContinuousMarkCorrelationError::AllocationFailed)?;
    for (index, radius_um) in radii_um.iter().copied().enumerate() {
        let product_sum = canonical_zero(shell_sums[index] + shell_corrections[index]);
        let correlation = (shell_counts[index] > 0)
            .then(|| canonical_zero(product_sum / shell_counts[index] as f64 / normalization));
        values.push(correlation.unwrap_or(0.0));
        eligible.push(correlation.is_some());
        points.push(ContinuousMarkCorrelationPoint {
            radius_um,
            shell_lower_um: if index == 0 { 0.0 } else { radii_um[index - 1] },
            status: if correlation.is_some() {
                ContinuousMarkCorrelationPointStatus::Available
            } else {
                ContinuousMarkCorrelationPointStatus::NoPairsInShell
            },
            directed_pairs_in_shell: shell_counts[index],
            mark_product_sum_in_shell: product_sum,
            correlation,
            inference_eligible: false,
            lower_correlation: None,
            upper_correlation: None,
        });
    }
    Ok(Evaluation {
        points,
        values,
        eligible,
    })
}

fn attach_envelope(
    points: &mut [ContinuousMarkCorrelationPoint],
    observed: &[f64],
    simulations: &[Vec<f64>],
    eligible: &[bool],
    alpha: f64,
) -> Result<Option<ContinuousMarkCorrelationComponentInference>, ContinuousMarkCorrelationError> {
    if !eligible.iter().any(|value| *value) {
        return Ok(None);
    }
    let envelope =
        GlobalEnvelope::from_curves_with_eligibility(observed, simulations, alpha, eligible)
            .map_err(dependency)?;
    for (index, point) in points.iter_mut().enumerate() {
        if eligible[index] {
            point.inference_eligible = true;
            point.lower_correlation = Some(canonical_zero(envelope.lower[index]));
            point.upper_correlation = Some(canonical_zero(envelope.upper[index]));
        }
    }
    Ok(Some(ContinuousMarkCorrelationComponentInference {
        p_global: canonical_zero(envelope.p_global),
        erl_depth: canonical_zero(envelope.erl_depth),
        critical_depth: canonical_zero(envelope.critical_depth),
        eligible_radius_count: eligible.iter().filter(|value| **value).count(),
    }))
}

pub(crate) fn configuration_digest(config: &ContinuousMarkCorrelationConfig) -> ContentDigest {
    let mut fields = vec![
        b"marklab-continuous-mark-correlation-config-v1".to_vec(),
        config.mark_id.as_str().as_bytes().to_vec(),
    ];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        b"global_arithmetic_mean_product".to_vec(),
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

fn compensated_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        compensated_add(&mut sum, &mut correction, value);
    }
    sum + correction
}

pub(crate) fn compensated_add(sum: &mut f64, correction: &mut f64, value: f64) {
    let updated = *sum + value;
    *correction += if sum.abs() >= value.abs() {
        (*sum - updated) + value
    } else {
        (value - updated) + *sum
    };
    *sum = updated;
}

fn pair_plan_error(error: MarkPairPlanError) -> ContinuousMarkCorrelationError {
    match error {
        MarkPairPlanError::DirectedPairLimitExceeded { maximum } => {
            ContinuousMarkCorrelationError::DirectedPairLimitExceeded { maximum }
        }
        MarkPairPlanError::RetainedByteLimitExceeded { required, maximum } => {
            ContinuousMarkCorrelationError::RetainedByteLimitExceeded { required, maximum }
        }
        MarkPairPlanError::AllocationFailed => ContinuousMarkCorrelationError::AllocationFailed,
        MarkPairPlanError::SizeOverflow => ContinuousMarkCorrelationError::SizeOverflow,
        MarkPairPlanError::Dependency(reason) => {
            ContinuousMarkCorrelationError::Dependency { reason }
        }
    }
}

fn dependency(error: impl std::fmt::Display) -> ContinuousMarkCorrelationError {
    ContinuousMarkCorrelationError::Dependency {
        reason: error.to_string(),
    }
}

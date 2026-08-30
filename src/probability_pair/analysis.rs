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
    points: Vec<ProbabilityPairPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
}

/// Estimate positive-positive binary mark connection from exact expected probability contributions.
pub fn probability_mark_connection(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &ProbabilityPairConfig,
) -> Result<ProbabilityPairResult, ProbabilityPairError> {
    let frame = window
        .coordinate_frame_id()
        .ok_or(ProbabilityPairError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id() {
        return Err(ProbabilityPairError::CoordinateFrameMismatch);
    }
    let table = input
        .mark_table()
        .ok_or(ProbabilityPairError::MissingProbabilityMark)?;
    let probabilities = table
        .probability_values(&config.mark_id)
        .ok_or(ProbabilityPairError::MissingProbabilityMark)?;
    let measurement_status = table
        .measurement_status(&config.mark_id)
        .ok_or(ProbabilityPairError::MissingProbabilityMark)?;
    if probabilities.len() > config.limits.maximum_points {
        return Err(ProbabilityPairError::PointLimitExceeded {
            observed: probabilities.len(),
            maximum: config.limits.maximum_points,
        });
    }
    let (positive_mass, negative_mass, ordered_positive_pair_mass) =
        effective_masses(probabilities);
    if ordered_positive_pair_mass <= 0.0 {
        return Err(ProbabilityPairError::InsufficientEffectivePositivePairMass);
    }
    if negative_mass <= 0.0 {
        return Err(ProbabilityPairError::NoEffectiveNegativeMass);
    }

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
        .ok_or(ProbabilityPairError::SizeOverflow)?;
    if required_evaluations > config.limits.maximum_permutation_pair_evaluations {
        return Err(ProbabilityPairError::PermutationPairLimitExceeded {
            required: required_evaluations,
            maximum: config.limits.maximum_permutation_pair_evaluations,
        });
    }
    let matrix_value_bytes = config
        .permutations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(ProbabilityPairError::SizeOverflow)?;
    let matrix_descriptor_bytes = config
        .permutations
        .checked_mul(std::mem::size_of::<Vec<f64>>())
        .ok_or(ProbabilityPairError::SizeOverflow)?;
    let radius_work_bytes = config
        .radii_um
        .len()
        .checked_mul(
            2 * std::mem::size_of::<ProbabilityPairPoint>()
                + 4 * std::mem::size_of::<f64>()
                + 2 * std::mem::size_of::<usize>()
                + 2 * std::mem::size_of::<bool>(),
        )
        .ok_or(ProbabilityPairError::SizeOverflow)?;
    let permutation_work_bytes = probabilities
        .len()
        .checked_mul(
            4 * std::mem::size_of::<usize>()
                + std::mem::size_of::<f32>()
                + std::mem::size_of::<bool>(),
        )
        .ok_or(ProbabilityPairError::SizeOverflow)?;
    let erl_bytes = erl_workspace_bytes(config.permutations, config.radii_um.len())
        .ok_or(ProbabilityPairError::SizeOverflow)?;
    let retained_bytes = plan
        .retained_bytes
        .checked_add(matrix_value_bytes)
        .and_then(|value| value.checked_add(matrix_descriptor_bytes))
        .and_then(|value| value.checked_add(radius_work_bytes))
        .and_then(|value| value.checked_add(permutation_work_bytes))
        .and_then(|value| value.checked_add(erl_bytes))
        .ok_or(ProbabilityPairError::SizeOverflow)?;
    if retained_bytes > config.limits.maximum_retained_bytes {
        return Err(ProbabilityPairError::RetainedByteLimitExceeded {
            required: retained_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }

    let mut observed = evaluate(probabilities, &plan, &config.radii_um)?;
    let design = InferenceDesign::random_labeling(
        probabilities.len(),
        config.permutations,
        config.seed,
        InferenceAlternative::TwoSided,
    )
    .map_err(dependency)?;
    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(config.permutations)
        .map_err(|_| ProbabilityPairError::AllocationFailed)?;
    for replicate in 0..config.permutations {
        let indices = design.permuted_indices(replicate).map_err(dependency)?;
        let mut permuted = Vec::new();
        permuted
            .try_reserve_exact(indices.len())
            .map_err(|_| ProbabilityPairError::AllocationFailed)?;
        permuted.extend(indices.iter().map(|index| probabilities[*index]));
        simulated.push(evaluate(&permuted, &plan, &config.radii_um)?.values);
    }
    let connection = attach_envelope(
        &mut observed.points,
        &observed.values,
        &simulated,
        &observed.eligible,
        config.alpha,
    )?;
    let denominator = probabilities
        .len()
        .checked_mul(probabilities.len().saturating_sub(1))
        .ok_or(ProbabilityPairError::SizeOverflow)?;
    Ok(ProbabilityPairResult {
        mark_id: config.mark_id.clone(),
        measurement_status,
        coordinate_frame_id: frame.clone(),
        window: window_summary(window.descriptor()),
        normalization: "expected_positive_positive_pairs_per_eligible_directed_pair".into(),
        effective_positive_mass: canonical_zero(positive_mass),
        effective_negative_mass: canonical_zero(negative_mass),
        expected_random_label_connection: canonical_zero(
            ordered_positive_pair_mass / denominator as f64,
        ),
        geometry: ProbabilityPairGeometrySummary {
            point_count: probabilities.len(),
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
        inference: ProbabilityPairInferenceSummary {
            null_model: "random_labeling".into(),
            permutation_unit: "complete_probability_row".into(),
            probability_mode: "exact_expected_contributions".into(),
            alternative: "two_sided".into(),
            multiplicity: "single_connection_erl_family".into(),
            connection,
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
    probabilities: &[f32],
    plan: &MarkPairPlan,
    radii_um: &[f64],
) -> Result<Evaluation, ProbabilityPairError> {
    let mut shell_counts = vec![0_usize; radii_um.len()];
    let mut shell_mass = vec![0.0_f64; radii_um.len()];
    for pair in &plan.pairs {
        let shell = radii_um.partition_point(|radius| *radius < pair.distance_um);
        let eligible_end = radii_um
            .partition_point(|radius| *radius <= plan.geometry.boundary_distances()[pair.source]);
        if shell >= eligible_end || shell >= radii_um.len() {
            continue;
        }
        shell_counts[shell] = shell_counts[shell]
            .checked_add(1)
            .ok_or(ProbabilityPairError::SizeOverflow)?;
        shell_mass[shell] +=
            f64::from(probabilities[pair.source]) * f64::from(probabilities[pair.target]);
    }
    let mut points = Vec::new();
    let mut values = Vec::new();
    let mut eligible = Vec::new();
    points
        .try_reserve_exact(radii_um.len())
        .and_then(|()| values.try_reserve_exact(radii_um.len()))
        .and_then(|()| eligible.try_reserve_exact(radii_um.len()))
        .map_err(|_| ProbabilityPairError::AllocationFailed)?;
    for (index, radius_um) in radii_um.iter().copied().enumerate() {
        let connection = (shell_counts[index] > 0)
            .then(|| canonical_zero(shell_mass[index] / shell_counts[index] as f64));
        values.push(connection.unwrap_or(0.0));
        eligible.push(connection.is_some());
        points.push(ProbabilityPairPoint {
            radius_um,
            shell_lower_um: if index == 0 { 0.0 } else { radii_um[index - 1] },
            status: if connection.is_some() {
                ProbabilityPairPointStatus::Available
            } else {
                ProbabilityPairPointStatus::NoPairsInShell
            },
            directed_pairs_in_shell: shell_counts[index],
            expected_positive_pairs_in_shell: canonical_zero(shell_mass[index]),
            connection_probability: connection,
            connection_inference_eligible: false,
            lower_connection: None,
            upper_connection: None,
        });
    }
    Ok(Evaluation {
        points,
        values,
        eligible,
    })
}

fn attach_envelope(
    points: &mut [ProbabilityPairPoint],
    observed: &[f64],
    simulations: &[Vec<f64>],
    eligible: &[bool],
    alpha: f64,
) -> Result<Option<ProbabilityPairComponentInference>, ProbabilityPairError> {
    if !eligible.iter().any(|value| *value) {
        return Ok(None);
    }
    let envelope =
        GlobalEnvelope::from_curves_with_eligibility(observed, simulations, alpha, eligible)
            .map_err(dependency)?;
    for (index, point) in points.iter_mut().enumerate() {
        if eligible[index] {
            point.connection_inference_eligible = true;
            point.lower_connection = Some(canonical_zero(envelope.lower[index]));
            point.upper_connection = Some(canonical_zero(envelope.upper[index]));
        }
    }
    Ok(Some(ProbabilityPairComponentInference {
        p_global: canonical_zero(envelope.p_global),
        erl_depth: canonical_zero(envelope.erl_depth),
        critical_depth: canonical_zero(envelope.critical_depth),
        eligible_radius_count: eligible.iter().filter(|value| **value).count(),
    }))
}

fn effective_masses(probabilities: &[f32]) -> (f64, f64, f64) {
    let positive = probabilities
        .iter()
        .map(|value| f64::from(*value))
        .sum::<f64>();
    let square = probabilities
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>();
    let negative = probabilities.len() as f64 - positive;
    (positive, negative, (positive * positive - square).max(0.0))
}

pub(crate) fn configuration_digest(config: &ProbabilityPairConfig) -> ContentDigest {
    let mut fields = vec![
        b"marklab-probability-pair-config-v1".to_vec(),
        config.mark_id.as_str().as_bytes().to_vec(),
    ];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
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

fn pair_plan_error(error: MarkPairPlanError) -> ProbabilityPairError {
    match error {
        MarkPairPlanError::DirectedPairLimitExceeded { maximum } => {
            ProbabilityPairError::DirectedPairLimitExceeded { maximum }
        }
        MarkPairPlanError::RetainedByteLimitExceeded { required, maximum } => {
            ProbabilityPairError::RetainedByteLimitExceeded { required, maximum }
        }
        MarkPairPlanError::AllocationFailed => ProbabilityPairError::AllocationFailed,
        MarkPairPlanError::SizeOverflow => ProbabilityPairError::SizeOverflow,
        MarkPairPlanError::Dependency(reason) => ProbabilityPairError::Dependency { reason },
    }
}

fn dependency(error: impl std::fmt::Display) -> ProbabilityPairError {
    ProbabilityPairError::Dependency {
        reason: error.to_string(),
    }
}

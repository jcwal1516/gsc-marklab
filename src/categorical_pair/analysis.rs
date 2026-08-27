use marklab_cohort::{InferenceAlternative, InferenceDesign};
use marklab_workflow::ContentDigest;

use crate::{
    classical::window_summary,
    mark_pair_plan::{build_mark_pair_plan, MarkPairPlan, MarkPairPlanError},
    permutation::envelopes::GlobalEnvelope,
    DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
};

use super::types::*;

struct Evaluation {
    points: Vec<CategoricalPairPoint>,
    connection_values: Vec<f64>,
    cross_k_values: Vec<f64>,
    connection_eligible: Vec<bool>,
    cross_k_eligible: Vec<bool>,
}

/// Estimate directed categorical mark connection and standard-border cross-K with random labeling.
pub fn categorical_mark_connection_cross_k(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &CategoricalPairConfig,
) -> Result<CategoricalPairResult, CategoricalPairError> {
    let frame = window
        .coordinate_frame_id()
        .ok_or(CategoricalPairError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id() {
        return Err(CategoricalPairError::CoordinateFrameMismatch);
    }
    let mark_id = ScalarMarkId::new("histologic_compartment").map_err(dependency)?;
    let table = input
        .mark_table()
        .ok_or(CategoricalPairError::MissingCategoricalMark)?;
    let codes = table
        .categorical_values(&mark_id)
        .ok_or(CategoricalPairError::MissingCategoricalMark)?;
    let levels = table
        .categorical_levels(&mark_id)
        .ok_or(CategoricalPairError::MissingCategoricalMark)?;
    let measurement_status = table
        .measurement_status(&mark_id)
        .ok_or(CategoricalPairError::MissingCategoricalMark)?;
    let source_code = resolve_level(levels, &config.source_level)?;
    let target_code = resolve_level(levels, &config.target_level)?;
    let source_count = codes.iter().filter(|code| **code == source_code).count();
    let target_count = codes.iter().filter(|code| **code == target_code).count();
    if source_count == 0 {
        return Err(CategoricalPairError::EmptyLevel {
            level: config.source_level.clone(),
        });
    }
    if target_count == 0 {
        return Err(CategoricalPairError::EmptyLevel {
            level: config.target_level.clone(),
        });
    }
    if codes.len() > config.limits.maximum_points {
        return Err(CategoricalPairError::PointLimitExceeded {
            observed: codes.len(),
            maximum: config.limits.maximum_points,
        });
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
        .ok_or(CategoricalPairError::SizeOverflow)?;
    if required_evaluations > config.limits.maximum_permutation_pair_evaluations {
        return Err(CategoricalPairError::PermutationPairLimitExceeded {
            required: required_evaluations,
            maximum: config.limits.maximum_permutation_pair_evaluations,
        });
    }
    let matrix_bytes = config
        .permutations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(CategoricalPairError::SizeOverflow)?;
    let permutation_work_bytes = codes
        .len()
        .checked_mul(
            4 * std::mem::size_of::<usize>()
                + std::mem::size_of::<u32>()
                + std::mem::size_of::<bool>(),
        )
        .and_then(|value| {
            value.checked_add(
                config
                    .radii_um
                    .len()
                    .checked_mul(16 * std::mem::size_of::<usize>())?,
            )
        })
        .ok_or(CategoricalPairError::SizeOverflow)?;
    let retained_bytes = plan
        .retained_bytes
        .checked_add(matrix_bytes)
        .and_then(|value| value.checked_add(permutation_work_bytes))
        .ok_or(CategoricalPairError::SizeOverflow)?;
    if retained_bytes > config.limits.maximum_retained_bytes {
        return Err(CategoricalPairError::RetainedByteLimitExceeded {
            required: retained_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }

    let mut observed = evaluate(
        codes,
        source_code,
        target_code,
        target_count,
        window.area_um2(),
        &plan,
        &config.radii_um,
    )?;
    let design = InferenceDesign::random_labeling(
        codes.len(),
        config.permutations,
        config.seed,
        InferenceAlternative::TwoSided,
    )
    .map_err(dependency)?;
    let mut simulated_connection = Vec::new();
    let mut simulated_cross_k = Vec::new();
    simulated_connection
        .try_reserve_exact(config.permutations)
        .and_then(|()| simulated_cross_k.try_reserve_exact(config.permutations))
        .map_err(|_| CategoricalPairError::AllocationFailed)?;
    let mut connection_eligible = observed.connection_eligible.clone();
    let mut cross_k_eligible = observed.cross_k_eligible.clone();
    for replicate in 0..config.permutations {
        let indices = design.permuted_indices(replicate).map_err(dependency)?;
        let mut permuted = Vec::new();
        permuted
            .try_reserve_exact(indices.len())
            .map_err(|_| CategoricalPairError::AllocationFailed)?;
        permuted.extend(indices.iter().map(|index| codes[*index]));
        let evaluated = evaluate(
            &permuted,
            source_code,
            target_code,
            target_count,
            window.area_um2(),
            &plan,
            &config.radii_um,
        )?;
        intersect(&mut connection_eligible, &evaluated.connection_eligible);
        intersect(&mut cross_k_eligible, &evaluated.cross_k_eligible);
        simulated_connection.push(evaluated.connection_values);
        simulated_cross_k.push(evaluated.cross_k_values);
    }
    let connection = attach_envelope(
        &mut observed.points,
        Component::Connection,
        &observed.connection_values,
        &simulated_connection,
        &connection_eligible,
        config.alpha,
    )?;
    let cross_k = attach_envelope(
        &mut observed.points,
        Component::CrossK,
        &observed.cross_k_values,
        &simulated_cross_k,
        &cross_k_eligible,
        config.alpha,
    )?;
    let denominator = codes
        .len()
        .checked_mul(codes.len().saturating_sub(1))
        .ok_or(CategoricalPairError::SizeOverflow)?;
    let expected_connection = source_count
        .checked_mul(target_count)
        .ok_or(CategoricalPairError::SizeOverflow)? as f64
        / denominator as f64;
    Ok(CategoricalPairResult {
        mark_id,
        measurement_status,
        coordinate_frame_id: frame.clone(),
        window: window_summary(window.descriptor()),
        source_level: config.source_level.clone(),
        target_level: config.target_level.clone(),
        source_count,
        target_count,
        expected_random_label_connection: canonical_zero(expected_connection),
        geometry: CategoricalPairGeometrySummary {
            point_count: codes.len(),
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
        inference: CategoricalPairInferenceSummary {
            null_model: "random_labeling".into(),
            permutation_unit: "complete_categorical_row".into(),
            alternative: "two_sided".into(),
            multiplicity: "separate_componentwise_erl_families".into(),
            connection,
            cross_k,
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
    codes: &[u32],
    source_code: u32,
    target_code: u32,
    target_count: usize,
    area_um2: f64,
    plan: &MarkPairPlan,
    radii_um: &[f64],
) -> Result<Evaluation, CategoricalPairError> {
    let width = radii_um
        .len()
        .checked_add(1)
        .ok_or(CategoricalPairError::SizeOverflow)?;
    let mut eligible_ends = vec![0_usize; width];
    let mut cross_starts = vec![0_usize; width];
    let mut cross_ends = vec![0_usize; width];
    let mut shell_total = vec![0_usize; radii_um.len()];
    let mut shell_match = vec![0_usize; radii_um.len()];
    let source_count = codes.iter().filter(|code| **code == source_code).count();
    for (row, code) in codes.iter().copied().enumerate() {
        if code == source_code {
            let end = radii_um
                .partition_point(|radius| *radius <= plan.geometry.boundary_distances()[row]);
            eligible_ends[end] = eligible_ends[end]
                .checked_add(1)
                .ok_or(CategoricalPairError::SizeOverflow)?;
        }
    }
    for pair in &plan.pairs {
        let start = radii_um.partition_point(|radius| *radius < pair.distance_um);
        let end = radii_um
            .partition_point(|radius| *radius <= plan.geometry.boundary_distances()[pair.source]);
        if start >= end || start >= radii_um.len() {
            continue;
        }
        shell_total[start] = shell_total[start]
            .checked_add(1)
            .ok_or(CategoricalPairError::SizeOverflow)?;
        if codes[pair.source] == source_code && codes[pair.target] == target_code {
            shell_match[start] = shell_match[start]
                .checked_add(1)
                .ok_or(CategoricalPairError::SizeOverflow)?;
            cross_starts[start] = cross_starts[start]
                .checked_add(1)
                .ok_or(CategoricalPairError::SizeOverflow)?;
            cross_ends[end] = cross_ends[end]
                .checked_add(1)
                .ok_or(CategoricalPairError::SizeOverflow)?;
        }
    }

    let mut eligible_sources = source_count;
    let mut active_cross_pairs = 0_usize;
    let mut points = Vec::new();
    let mut connection_values = Vec::new();
    let mut cross_k_values = Vec::new();
    let mut connection_eligible = Vec::new();
    let mut cross_k_eligible = Vec::new();
    for output in [&mut connection_values, &mut cross_k_values] {
        output
            .try_reserve_exact(radii_um.len())
            .map_err(|_| CategoricalPairError::AllocationFailed)?;
    }
    points
        .try_reserve_exact(radii_um.len())
        .and_then(|()| connection_eligible.try_reserve_exact(radii_um.len()))
        .and_then(|()| cross_k_eligible.try_reserve_exact(radii_um.len()))
        .map_err(|_| CategoricalPairError::AllocationFailed)?;
    for (index, radius_um) in radii_um.iter().copied().enumerate() {
        eligible_sources = eligible_sources
            .checked_sub(eligible_ends[index])
            .ok_or(CategoricalPairError::SizeOverflow)?;
        active_cross_pairs = active_cross_pairs
            .checked_sub(cross_ends[index])
            .and_then(|value| value.checked_add(cross_starts[index]))
            .ok_or(CategoricalPairError::SizeOverflow)?;
        let connection = (shell_total[index] > 0)
            .then(|| canonical_zero(shell_match[index] as f64 / shell_total[index] as f64));
        let cross_k = (eligible_sources > 0 && target_count > 0).then(|| {
            canonical_zero(
                area_um2 * active_cross_pairs as f64 / (eligible_sources * target_count) as f64,
            )
        });
        connection_values.push(connection.unwrap_or(0.0));
        cross_k_values.push(cross_k.unwrap_or(0.0));
        connection_eligible.push(connection.is_some());
        cross_k_eligible.push(cross_k.is_some());
        points.push(CategoricalPairPoint {
            radius_um,
            shell_lower_um: if index == 0 { 0.0 } else { radii_um[index - 1] },
            status: if connection.is_some() {
                CategoricalPairPointStatus::Available
            } else {
                CategoricalPairPointStatus::NoPairsInShell
            },
            eligible_source_centers: eligible_sources,
            directed_source_target_pairs: active_cross_pairs,
            directed_pairs_in_shell: shell_total[index],
            source_target_pairs_in_shell: shell_match[index],
            connection_probability: connection,
            cross_k,
            theoretical_cross_k: std::f64::consts::PI * radius_um * radius_um,
            connection_inference_eligible: false,
            lower_connection: None,
            upper_connection: None,
            cross_k_inference_eligible: false,
            lower_cross_k: None,
            upper_cross_k: None,
        });
    }
    Ok(Evaluation {
        points,
        connection_values,
        cross_k_values,
        connection_eligible,
        cross_k_eligible,
    })
}

#[derive(Clone, Copy)]
enum Component {
    Connection,
    CrossK,
}

fn attach_envelope(
    points: &mut [CategoricalPairPoint],
    component: Component,
    observed: &[f64],
    simulations: &[Vec<f64>],
    eligible: &[bool],
    alpha: f64,
) -> Result<Option<CategoricalPairComponentInference>, CategoricalPairError> {
    if !eligible.iter().any(|value| *value) {
        return Ok(None);
    }
    let envelope =
        GlobalEnvelope::from_curves_with_eligibility(observed, simulations, alpha, eligible)
            .map_err(dependency)?;
    for (index, point) in points.iter_mut().enumerate() {
        if !eligible[index] {
            continue;
        }
        let lower = canonical_zero(envelope.lower[index]);
        let upper = canonical_zero(envelope.upper[index]);
        match component {
            Component::Connection => {
                point.connection_inference_eligible = true;
                point.lower_connection = Some(lower);
                point.upper_connection = Some(upper);
            }
            Component::CrossK => {
                point.cross_k_inference_eligible = true;
                point.lower_cross_k = Some(lower);
                point.upper_cross_k = Some(upper);
            }
        }
    }
    Ok(Some(CategoricalPairComponentInference {
        p_global: canonical_zero(envelope.p_global),
        erl_depth: canonical_zero(envelope.erl_depth),
        critical_depth: canonical_zero(envelope.critical_depth),
        eligible_radius_count: eligible.iter().filter(|value| **value).count(),
    }))
}

pub(crate) fn configuration_digest(config: &CategoricalPairConfig) -> ContentDigest {
    let mut fields = vec![b"marklab-categorical-pair-config-v1".to_vec()];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        config.source_level.as_bytes().to_vec(),
        config.target_level.as_bytes().to_vec(),
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

fn resolve_level(levels: &[String], requested: &str) -> Result<u32, CategoricalPairError> {
    levels
        .iter()
        .position(|level| level == requested)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| CategoricalPairError::UnknownLevel {
            level: requested.into(),
        })
}

fn intersect(target: &mut [bool], observed: &[bool]) {
    for (target, observed) in target.iter_mut().zip(observed) {
        *target &= *observed;
    }
}

fn dependency(error: impl std::fmt::Display) -> CategoricalPairError {
    CategoricalPairError::Dependency {
        reason: error.to_string(),
    }
}

fn pair_plan_error(error: MarkPairPlanError) -> CategoricalPairError {
    match error {
        MarkPairPlanError::DirectedPairLimitExceeded { maximum } => {
            CategoricalPairError::DirectedPairLimitExceeded { maximum }
        }
        MarkPairPlanError::RetainedByteLimitExceeded { required, maximum } => {
            CategoricalPairError::RetainedByteLimitExceeded { required, maximum }
        }
        MarkPairPlanError::AllocationFailed => CategoricalPairError::AllocationFailed,
        MarkPairPlanError::SizeOverflow => CategoricalPairError::SizeOverflow,
        MarkPairPlanError::Dependency(reason) => CategoricalPairError::Dependency { reason },
    }
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

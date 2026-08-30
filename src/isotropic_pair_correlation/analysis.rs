use marklab_workflow::ContentDigest;

use crate::{
    classical::{
        sample_conditional_csr, window_summary, ClassicalInferenceSummary, ClassicalNullDesign,
        ClassicalNullModel, ClassicalRandomizationUnit, ClassicalSpatialLimits,
        SpatialGeometryPlan2D,
    },
    common::seeds::{derive_seed, SeedEndpoint},
    isotropic_spatial::edge::IsotropicArcBudget,
    mark_pair_plan::erl_workspace_bytes,
    pair_correlation::epanechnikov_weight,
    permutation::envelopes::GlobalEnvelope,
    IsotropicSpatialError, ObservationWindow2D, PairCorrelationKernel, PairCorrelationPointStatus,
    Pattern,
};

use super::types::*;

struct Evaluation {
    points: Vec<IsotropicPairCorrelationPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
}

/// Estimate exact visible-arc isotropic Epanechnikov homogeneous g.
pub fn analyze_isotropic_pair_correlation(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &IsotropicPairCorrelationConfig,
) -> Result<IsotropicPairCorrelationResult, IsotropicSpatialError> {
    validate_pattern_shape(pattern)?;
    if pattern.len() > config.limits.maximum_points {
        return Err(IsotropicSpatialError::PointLimitExceeded {
            observed: pattern.len(),
            maximum: config.limits.maximum_points,
        });
    }
    let estimated_storage_bytes = retained_byte_estimate(pattern.len(), window, config)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(IsotropicSpatialError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }
    let classical_limits = ClassicalSpatialLimits::new(
        config.limits.maximum_points,
        config.limits.maximum_radii,
        config.limits.maximum_pair_visits,
        config.limits.maximum_csr_draws,
        config.limits.maximum_retained_bytes,
    )
    .map_err(invalid_pattern)?;
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, classical_limits)
            .map_err(invalid_pattern)?;
    let null_design = ClassicalNullDesign {
        null_model: ClassicalNullModel::HomogeneousCsrConditionalOnCount,
        randomization_unit: ClassicalRandomizationUnit::WholeLocationPattern,
        conditioned_point_count: pattern.len(),
        simulations: config.simulations,
        seed: config.seed,
        alpha: config.alpha,
    };
    let maximum_query_radius_um = config.radii_um[config.radii_um.len() - 1] + config.bandwidth_um;
    let configuration = IsotropicPairCorrelationConfigurationSummary {
        radius_count: config.radii_um.len(),
        logical_digest: isotropic_pair_correlation_configuration_digest(config).to_string(),
        limits: config.limits,
    };
    let mut geometry = IsotropicPairCorrelationGeometrySummary {
        point_count: pattern.len(),
        maximum_query_radius_um,
        observed_pair_visits: 0,
        total_pair_visits: 0,
        observed_visible_arc_evaluations: 0,
        total_visible_arc_evaluations: 0,
        total_arc_segment_tests: 0,
        total_arc_membership_queries: 0,
        maximum_boundary_intersection_angles: 0,
        estimated_storage_bytes,
        logical_digest: observed_plan.logical_digest().to_string(),
        execution_mode: "exact".into(),
        duplicate_policy: "reject".into(),
        visible_arc_owner: "observation_window_2d_segment_circle_partition".into(),
        pair_traversal:
            "exact_streaming_unordered_compact_support_pairs_with_two_directed_arc_weights".into(),
    };
    if pattern.len() < 2 {
        return Ok(IsotropicPairCorrelationResult {
            case_id: pattern.meta.case_id.clone(),
            timepoint: pattern.meta.timepoint.clone(),
            correction: "isotropic".into(),
            kernel: PairCorrelationKernel::Epanechnikov,
            bandwidth_um: config.bandwidth_um,
            window: window_summary(window.descriptor()),
            geometry,
            configuration,
            null_design,
            randomization_unit: "whole_location_pattern".into(),
            curve: unavailable_curve(&config.radii_um),
            inference: None,
            csr_candidate_draws: 0,
        });
    }

    let mut budget = IsotropicArcBudget::new(config.limits, window);
    let observed = evaluate(&pattern.x_um, &pattern.y_um, window, config, &mut budget)?;
    geometry.observed_pair_visits = budget.pair_visits;
    geometry.observed_visible_arc_evaluations = budget.visible_arc_evaluations;
    let mut curve = observed.points;
    let mut jointly_eligible = observed.eligible;
    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(config.simulations)
        .map_err(|_| IsotropicSpatialError::AllocationFailed)?;
    let mut csr_candidate_draws = 0_usize;
    for simulation in 0..config.simulations {
        let seed = derive_seed(
            config.seed,
            SeedEndpoint::IsotropicPairCorrelationCsr,
            simulation,
        );
        let (x, y) = sample_conditional_csr(
            window,
            pattern.len(),
            seed,
            &mut csr_candidate_draws,
            config.limits.maximum_csr_draws,
        )
        .map_err(|error| IsotropicSpatialError::Csr {
            reason: error.to_string(),
        })?;
        SpatialGeometryPlan2D::new(&x, &y, window, classical_limits).map_err(|error| {
            IsotropicSpatialError::Csr {
                reason: error.to_string(),
            }
        })?;
        let evaluated = evaluate(&x, &y, window, config, &mut budget)?;
        for (joint, current) in jointly_eligible.iter_mut().zip(&evaluated.eligible) {
            *joint &= *current;
        }
        simulated.push(evaluated.values);
    }
    let inference = if jointly_eligible.iter().any(|eligible| *eligible) {
        let observed_values = curve
            .iter()
            .map(|point| point.g.unwrap_or(0.0))
            .collect::<Vec<_>>();
        let envelope = GlobalEnvelope::from_curves_with_eligibility(
            &observed_values,
            &simulated,
            config.alpha,
            &jointly_eligible,
        )
        .map_err(|error| IsotropicSpatialError::Inference {
            reason: error.to_string(),
        })?;
        for (index, point) in curve.iter_mut().enumerate() {
            if jointly_eligible[index] {
                point.inference_eligible = true;
                point.lower_g = Some(canonical_zero(envelope.lower[index]));
                point.upper_g = Some(canonical_zero(envelope.upper[index]));
            }
        }
        Some(ClassicalInferenceSummary {
            p_global: canonical_zero(envelope.p_global),
            erl_depth: canonical_zero(envelope.erl_depth),
            critical_depth: canonical_zero(envelope.critical_depth),
            simulations: envelope.n_permutations,
            eligible_radius_count: jointly_eligible.iter().filter(|value| **value).count(),
        })
    } else {
        None
    };
    geometry.total_pair_visits = budget.pair_visits;
    geometry.total_visible_arc_evaluations = budget.visible_arc_evaluations;
    geometry.total_arc_segment_tests = budget.arc_segment_tests;
    geometry.total_arc_membership_queries = budget.arc_membership_queries;
    geometry.maximum_boundary_intersection_angles = budget.maximum_intersection_angles;
    Ok(IsotropicPairCorrelationResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        correction: "isotropic".into(),
        kernel: PairCorrelationKernel::Epanechnikov,
        bandwidth_um: config.bandwidth_um,
        window: window_summary(window.descriptor()),
        geometry,
        configuration,
        null_design,
        randomization_unit: "whole_location_pattern".into(),
        curve,
        inference,
        csr_candidate_draws,
    })
}

fn evaluate(
    x: &[f64],
    y: &[f64],
    window: &ObservationWindow2D,
    config: &IsotropicPairCorrelationConfig,
    budget: &mut IsotropicArcBudget,
) -> Result<Evaluation, IsotropicSpatialError> {
    let mut directed_pairs = vec![0_usize; config.radii_um.len()];
    let mut arc_evaluations = vec![0_usize; config.radii_um.len()];
    let mut fraction_sums = vec![0.0_f64; config.radii_um.len()];
    let mut weighted_inverse_sums = vec![0.0_f64; config.radii_um.len()];
    let maximum_query_radius = config.radii_um[config.radii_um.len() - 1] + config.bandwidth_um;
    for left in 0..x.len() {
        for right in (left + 1)..x.len() {
            budget.charge_pair()?;
            let distance = (x[right] - x[left]).hypot(y[right] - y[left]);
            if distance >= maximum_query_radius {
                continue;
            }
            let lower = distance - config.bandwidth_um;
            let upper = distance + config.bandwidth_um;
            let start = config.radii_um.partition_point(|radius| *radius <= lower);
            let end = config.radii_um.partition_point(|radius| *radius < upper);
            if start == end {
                continue;
            }
            let left_fraction =
                budget.visible_fraction(window, x[left], y[left], distance, left, right)?;
            let right_fraction =
                budget.visible_fraction(window, x[right], y[right], distance, right, left)?;
            for index in start..end {
                let kernel =
                    epanechnikov_weight(config.radii_um[index], distance, config.bandwidth_um)
                        .expect("strictly partitioned Epanechnikov support");
                directed_pairs[index] = directed_pairs[index]
                    .checked_add(2)
                    .ok_or(IsotropicSpatialError::SizeOverflow)?;
                arc_evaluations[index] = arc_evaluations[index]
                    .checked_add(2)
                    .ok_or(IsotropicSpatialError::SizeOverflow)?;
                fraction_sums[index] += left_fraction + right_fraction;
                weighted_inverse_sums[index] +=
                    kernel * (1.0 / left_fraction + 1.0 / right_fraction);
                if !fraction_sums[index].is_finite() || !weighted_inverse_sums[index].is_finite() {
                    return Err(IsotropicSpatialError::InvalidPattern {
                        reason: "isotropic pair-correlation accumulation is non-finite".into(),
                    });
                }
            }
        }
    }
    let denominator = x
        .len()
        .checked_mul(x.len().saturating_sub(1))
        .ok_or(IsotropicSpatialError::SizeOverflow)? as f64;
    let mut points = Vec::new();
    let mut values = Vec::new();
    let mut eligible = Vec::new();
    points
        .try_reserve_exact(config.radii_um.len())
        .and_then(|()| values.try_reserve_exact(config.radii_um.len()))
        .and_then(|()| eligible.try_reserve_exact(config.radii_um.len()))
        .map_err(|_| IsotropicSpatialError::AllocationFailed)?;
    for index in 0..config.radii_um.len() {
        let status = if directed_pairs[index] == 0 {
            PairCorrelationPointStatus::NoPairsInKernelSupport
        } else {
            PairCorrelationPointStatus::Available
        };
        let g = (status == PairCorrelationPointStatus::Available)
            .then(|| {
                window.area_um2() * weighted_inverse_sums[index]
                    / (std::f64::consts::TAU * config.radii_um[index] * denominator)
            })
            .filter(|value| value.is_finite())
            .map(canonical_zero);
        if status == PairCorrelationPointStatus::Available && g.is_none() {
            return Err(IsotropicSpatialError::InvalidPattern {
                reason: "isotropic pair-correlation normalization is non-finite".into(),
            });
        }
        values.push(g.unwrap_or(0.0));
        eligible.push(g.is_some());
        points.push(IsotropicPairCorrelationPoint {
            radius_um: config.radii_um[index],
            status,
            directed_pairs_in_support: directed_pairs[index],
            visible_arc_evaluations: arc_evaluations[index],
            visible_arc_fraction_sum: canonical_zero(fraction_sums[index]),
            inverse_visible_arc_weighted_kernel_sum: canonical_zero(weighted_inverse_sums[index]),
            g,
            theoretical_g: 1.0,
            inference_eligible: false,
            lower_g: None,
            upper_g: None,
        });
    }
    Ok(Evaluation {
        points,
        values,
        eligible,
    })
}

fn unavailable_curve(radii_um: &[f64]) -> Vec<IsotropicPairCorrelationPoint> {
    radii_um
        .iter()
        .copied()
        .map(|radius_um| IsotropicPairCorrelationPoint {
            radius_um,
            status: PairCorrelationPointStatus::NoPairsInKernelSupport,
            directed_pairs_in_support: 0,
            visible_arc_evaluations: 0,
            visible_arc_fraction_sum: 0.0,
            inverse_visible_arc_weighted_kernel_sum: 0.0,
            g: None,
            theoretical_g: 1.0,
            inference_eligible: false,
            lower_g: None,
            upper_g: None,
        })
        .collect()
}

fn validate_pattern_shape(pattern: &Pattern) -> Result<(), IsotropicSpatialError> {
    if pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|value| *value != 1)
    {
        return Err(IsotropicSpatialError::InvalidPattern {
            reason: "coordinate and validity arrays must match complete rows".into(),
        });
    }
    Ok(())
}

fn retained_byte_estimate(
    point_count: usize,
    window: &ObservationWindow2D,
    config: &IsotropicPairCorrelationConfig,
) -> Result<usize, IsotropicSpatialError> {
    let point_bytes = point_count
        .checked_mul(std::mem::size_of::<[f64; 2]>())
        .and_then(|value| value.checked_mul(7))
        .ok_or(IsotropicSpatialError::SizeOverflow)?;
    let window_bytes = window
        .descriptor()
        .vertex_count
        .checked_mul(std::mem::size_of::<[f64; 2]>())
        .and_then(|value| value.checked_mul(3))
        .and_then(|value| value.checked_add(window.boundary_storage_bytes()))
        .ok_or(IsotropicSpatialError::SizeOverflow)?;
    let curve_bytes = config
        .radii_um
        .len()
        .checked_mul(std::mem::size_of::<IsotropicPairCorrelationPoint>())
        .and_then(|value| value.checked_mul(5))
        .ok_or(IsotropicSpatialError::SizeOverflow)?;
    let matrix_bytes = config
        .simulations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(IsotropicSpatialError::SizeOverflow)?;
    let erl_bytes = erl_workspace_bytes(config.simulations, config.radii_um.len())
        .ok_or(IsotropicSpatialError::SizeOverflow)?;
    let arc_bytes = window
        .translation_segment_count()
        .checked_mul(2)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(IsotropicSpatialError::SizeOverflow)?;
    point_bytes
        .checked_add(window_bytes)
        .and_then(|value| value.checked_add(curve_bytes))
        .and_then(|value| value.checked_add(matrix_bytes))
        .and_then(|value| value.checked_add(erl_bytes))
        .and_then(|value| value.checked_add(arc_bytes))
        .ok_or(IsotropicSpatialError::SizeOverflow)
}

pub(crate) fn isotropic_pair_correlation_configuration_digest(
    config: &IsotropicPairCorrelationConfig,
) -> ContentDigest {
    let mut fields = vec![
        b"marklab-isotropic-pair-correlation-configuration-v1".to_vec(),
        (config.radii_um.len() as u128).to_be_bytes().to_vec(),
    ];
    fields.extend(
        config
            .radii_um
            .iter()
            .map(|radius| radius.to_bits().to_be_bytes().to_vec()),
    );
    fields.extend([
        config.bandwidth_um.to_bits().to_be_bytes().to_vec(),
        (config.simulations as u128).to_be_bytes().to_vec(),
        config.seed.to_be_bytes().to_vec(),
        config.alpha.to_bits().to_be_bytes().to_vec(),
        (config.limits.maximum_points as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_radii as u128).to_be_bytes().to_vec(),
        (config.limits.maximum_pair_visits as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_visible_arc_evaluations as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_arc_segment_tests as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_arc_membership_queries as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_csr_draws as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_retained_bytes as u128)
            .to_be_bytes()
            .to_vec(),
    ]);
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

fn invalid_pattern(error: impl std::fmt::Display) -> IsotropicSpatialError {
    IsotropicSpatialError::InvalidPattern {
        reason: error.to_string(),
    }
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

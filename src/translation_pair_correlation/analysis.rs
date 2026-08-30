use marklab_workflow::ContentDigest;

use crate::common::finite::canonical_zero;

use crate::{
    classical::{
        sample_conditional_csr, window_summary, ClassicalInferenceSummary, ClassicalNullDesign,
        ClassicalNullModel, ClassicalRandomizationUnit, ClassicalSpatialLimits,
        SpatialGeometryPlan2D,
    },
    common::seeds::{derive_seed, SeedEndpoint},
    mark_pair_plan::erl_workspace_bytes,
    pair_correlation::epanechnikov_weight,
    permutation::envelopes::GlobalEnvelope,
    translation_spatial::edge::{preflight_overlap_complexity, TranslationOverlapBudget},
    ObservationWindow2D, PairCorrelationKernel, PairCorrelationPointStatus, Pattern,
    TranslationSpatialError,
};

use super::types::*;

struct Evaluation {
    points: Vec<TranslationPairCorrelationPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
}

/// Estimate exact polygon translation-corrected Epanechnikov homogeneous g.
pub fn analyze_translation_pair_correlation(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &TranslationPairCorrelationConfig,
) -> Result<TranslationPairCorrelationResult, TranslationSpatialError> {
    validate_pattern_shape(pattern)?;
    if pattern.len() > config.limits.maximum_points {
        return Err(TranslationSpatialError::PointLimitExceeded {
            observed: pattern.len(),
            maximum: config.limits.maximum_points,
        });
    }
    preflight_overlap_complexity(window, config.limits)?;
    let estimated_storage_bytes = retained_byte_estimate(pattern.len(), window, config)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(TranslationSpatialError::RetainedByteLimitExceeded {
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
    let configuration = TranslationPairCorrelationConfigurationSummary {
        radius_count: config.radii_um.len(),
        logical_digest: translation_pair_correlation_configuration_digest(config).to_string(),
        limits: config.limits,
    };
    let mut geometry = TranslationPairCorrelationGeometrySummary {
        point_count: pattern.len(),
        maximum_query_radius_um,
        observed_pair_visits: 0,
        total_pair_visits: 0,
        observed_overlap_evaluations: 0,
        total_overlap_evaluations: 0,
        total_overlap_candidate_work: 0,
        maximum_overlap_output_vertices_observed: 0,
        estimated_storage_bytes,
        logical_digest: observed_plan.logical_digest().to_string(),
        execution_mode: "exact".into(),
        duplicate_policy: "reject".into(),
        overlap_owner: "observation_window_2d_exact_boolean_intersection".into(),
        pair_traversal: "exact_streaming_unordered_compact_support_pairs_with_both_ordered_weights"
            .into(),
    };
    if pattern.len() < 2 {
        return Ok(TranslationPairCorrelationResult {
            case_id: pattern.meta.case_id.clone(),
            timepoint: pattern.meta.timepoint.clone(),
            correction: "translation".into(),
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

    let mut budget = TranslationOverlapBudget::new(config.limits, window)?;
    let observed = evaluate(&pattern.x_um, &pattern.y_um, window, config, &mut budget)?;
    geometry.observed_pair_visits = budget.pair_visits;
    geometry.observed_overlap_evaluations = budget.overlap_evaluations;
    let mut curve = observed.points;
    let mut jointly_eligible = observed.eligible;
    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(config.simulations)
        .map_err(|_| TranslationSpatialError::AllocationFailed)?;
    let mut csr_candidate_draws = 0_usize;
    for simulation in 0..config.simulations {
        let seed = derive_seed(
            config.seed,
            SeedEndpoint::TranslationPairCorrelationCsr,
            simulation,
        );
        let (x, y) = sample_conditional_csr(
            window,
            pattern.len(),
            seed,
            &mut csr_candidate_draws,
            config.limits.maximum_csr_draws,
        )
        .map_err(|error| TranslationSpatialError::Csr {
            reason: error.to_string(),
        })?;
        SpatialGeometryPlan2D::new(&x, &y, window, classical_limits).map_err(|error| {
            TranslationSpatialError::Csr {
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
        .map_err(|error| TranslationSpatialError::Inference {
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
    geometry.total_overlap_evaluations = budget.overlap_evaluations;
    geometry.total_overlap_candidate_work = budget.overlap_candidate_work;
    geometry.maximum_overlap_output_vertices_observed = budget.maximum_output_vertices;
    Ok(TranslationPairCorrelationResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        correction: "translation".into(),
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
    config: &TranslationPairCorrelationConfig,
    budget: &mut TranslationOverlapBudget,
) -> Result<Evaluation, TranslationSpatialError> {
    let mut support_pairs = vec![0_usize; config.radii_um.len()];
    let mut overlap_evaluations = vec![0_usize; config.radii_um.len()];
    let mut overlap_area_sums = vec![0.0_f64; config.radii_um.len()];
    let mut weighted_kernel_sums = vec![0.0_f64; config.radii_um.len()];
    let maximum_query_radius = config.radii_um[config.radii_um.len() - 1] + config.bandwidth_um;
    for left in 0..x.len() {
        for right in (left + 1)..x.len() {
            budget.charge_pair()?;
            let displacement_x = x[right] - x[left];
            let displacement_y = y[right] - y[left];
            let distance = displacement_x.hypot(displacement_y);
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
            let overlap = budget.overlap(window, displacement_x, displacement_y, left, right)?;
            let ordered_translation_weight = 2.0 * window.area_um2() / overlap;
            if !ordered_translation_weight.is_finite() {
                return Err(TranslationSpatialError::InvalidPattern {
                    reason: "translation pair weight is non-finite".into(),
                });
            }
            for index in start..end {
                let kernel =
                    epanechnikov_weight(config.radii_um[index], distance, config.bandwidth_um)
                        .expect("strictly partitioned Epanechnikov support");
                support_pairs[index] = support_pairs[index]
                    .checked_add(1)
                    .ok_or(TranslationSpatialError::SizeOverflow)?;
                overlap_evaluations[index] = overlap_evaluations[index]
                    .checked_add(1)
                    .ok_or(TranslationSpatialError::SizeOverflow)?;
                overlap_area_sums[index] += overlap;
                weighted_kernel_sums[index] += ordered_translation_weight * kernel;
                if !overlap_area_sums[index].is_finite() || !weighted_kernel_sums[index].is_finite()
                {
                    return Err(TranslationSpatialError::InvalidPattern {
                        reason: "translation pair-correlation accumulation is non-finite".into(),
                    });
                }
            }
        }
    }
    let denominator_count = x
        .len()
        .checked_mul(x.len().saturating_sub(1))
        .ok_or(TranslationSpatialError::SizeOverflow)? as f64;
    let mut points = Vec::new();
    let mut values = Vec::new();
    let mut eligible = Vec::new();
    points
        .try_reserve_exact(config.radii_um.len())
        .and_then(|()| values.try_reserve_exact(config.radii_um.len()))
        .and_then(|()| eligible.try_reserve_exact(config.radii_um.len()))
        .map_err(|_| TranslationSpatialError::AllocationFailed)?;
    for index in 0..config.radii_um.len() {
        let status = if support_pairs[index] == 0 {
            PairCorrelationPointStatus::NoPairsInKernelSupport
        } else {
            PairCorrelationPointStatus::Available
        };
        let g = (status == PairCorrelationPointStatus::Available)
            .then(|| {
                window.area_um2() * weighted_kernel_sums[index]
                    / (std::f64::consts::TAU * config.radii_um[index] * denominator_count)
            })
            .filter(|value| value.is_finite())
            .map(canonical_zero);
        if status == PairCorrelationPointStatus::Available && g.is_none() {
            return Err(TranslationSpatialError::InvalidPattern {
                reason: "translation pair-correlation normalization is non-finite".into(),
            });
        }
        values.push(g.unwrap_or(0.0));
        eligible.push(g.is_some());
        points.push(TranslationPairCorrelationPoint {
            radius_um: config.radii_um[index],
            status,
            ordered_pairs_in_support: support_pairs[index]
                .checked_mul(2)
                .ok_or(TranslationSpatialError::SizeOverflow)?,
            overlap_evaluations: overlap_evaluations[index],
            translation_overlap_area_sum_um2: canonical_zero(overlap_area_sums[index]),
            translation_weighted_kernel_sum: canonical_zero(weighted_kernel_sums[index]),
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

fn unavailable_curve(radii_um: &[f64]) -> Vec<TranslationPairCorrelationPoint> {
    radii_um
        .iter()
        .copied()
        .map(|radius_um| TranslationPairCorrelationPoint {
            radius_um,
            status: PairCorrelationPointStatus::NoPairsInKernelSupport,
            ordered_pairs_in_support: 0,
            overlap_evaluations: 0,
            translation_overlap_area_sum_um2: 0.0,
            translation_weighted_kernel_sum: 0.0,
            g: None,
            theoretical_g: 1.0,
            inference_eligible: false,
            lower_g: None,
            upper_g: None,
        })
        .collect()
}

fn validate_pattern_shape(pattern: &Pattern) -> Result<(), TranslationSpatialError> {
    if pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|value| *value != 1)
    {
        return Err(TranslationSpatialError::InvalidPattern {
            reason: "coordinate and validity arrays must match complete rows".into(),
        });
    }
    Ok(())
}

fn retained_byte_estimate(
    point_count: usize,
    window: &ObservationWindow2D,
    config: &TranslationPairCorrelationConfig,
) -> Result<usize, TranslationSpatialError> {
    let point_bytes = point_count
        .checked_mul(std::mem::size_of::<[f64; 2]>())
        .and_then(|value| value.checked_mul(5))
        .ok_or(TranslationSpatialError::SizeOverflow)?;
    let window_bytes = window
        .descriptor()
        .vertex_count
        .checked_mul(std::mem::size_of::<[f64; 2]>())
        .and_then(|value| value.checked_mul(3))
        .and_then(|value| value.checked_add(window.boundary_storage_bytes()))
        .ok_or(TranslationSpatialError::SizeOverflow)?;
    let curve_bytes = config
        .radii_um
        .len()
        .checked_mul(std::mem::size_of::<TranslationPairCorrelationPoint>())
        .and_then(|value| value.checked_mul(5))
        .ok_or(TranslationSpatialError::SizeOverflow)?;
    let matrix_bytes = config
        .simulations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(TranslationSpatialError::SizeOverflow)?;
    let erl_bytes = erl_workspace_bytes(config.simulations, config.radii_um.len())
        .ok_or(TranslationSpatialError::SizeOverflow)?;
    let overlap_bytes = config
        .limits
        .maximum_overlap_output_vertices
        .checked_mul(std::mem::size_of::<[f64; 2]>())
        .and_then(|value| value.checked_mul(3))
        .ok_or(TranslationSpatialError::SizeOverflow)?;
    point_bytes
        .checked_add(window_bytes)
        .and_then(|value| value.checked_add(curve_bytes))
        .and_then(|value| value.checked_add(matrix_bytes))
        .and_then(|value| value.checked_add(erl_bytes))
        .and_then(|value| value.checked_add(overlap_bytes))
        .ok_or(TranslationSpatialError::SizeOverflow)
}

pub(crate) fn translation_pair_correlation_configuration_digest(
    config: &TranslationPairCorrelationConfig,
) -> ContentDigest {
    let mut fields = vec![
        b"marklab-translation-pair-correlation-configuration-v1".to_vec(),
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
        (config.limits.maximum_overlap_evaluations as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_overlap_candidate_work as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_overlap_output_vertices as u128)
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

fn invalid_pattern(error: impl std::fmt::Display) -> TranslationSpatialError {
    TranslationSpatialError::InvalidPattern {
        reason: error.to_string(),
    }
}

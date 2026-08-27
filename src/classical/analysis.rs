use std::collections::BTreeSet;

use marklab_workflow::ContentDigest;

use crate::{
    common::seeds::{derive_seed, splitmix64, SeedEndpoint},
    data::Pattern,
    geom::window::{ObservationWindow2D, ObservationWindowDescriptor},
    permutation::envelopes::GlobalEnvelope,
};

use super::{
    geometry::{BorderCounts, PairVisitBudget, SpatialGeometryPlan2D},
    types::*,
};

/// Compute one exact homogeneous border K/L workflow with conditional-CSR inference.
pub fn analyze_classical_spatial_pattern(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &ClassicalSpatialConfig,
) -> Result<ClassicalSpatialResult, ClassicalSpatialError> {
    validate_pattern_shape(pattern)?;
    let retained_estimate = retained_byte_estimate(pattern.len(), window, config)?;
    if retained_estimate > config.limits.maximum_retained_bytes {
        return Err(ClassicalSpatialError::RetainedByteLimitExceeded {
            required: retained_estimate,
            maximum: config.limits.maximum_retained_bytes,
        });
    }
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, config.limits)?;
    let null_design = ClassicalNullDesign {
        null_model: ClassicalNullModel::HomogeneousCsrConditionalOnCount,
        randomization_unit: ClassicalRandomizationUnit::WholeLocationPattern,
        conditioned_point_count: pattern.len(),
        simulations: config.simulations,
        seed: config.seed,
        alpha: config.alpha,
    };
    let configuration = ClassicalConfigurationSummary {
        radius_count: config.radii_um.len(),
        logical_digest: classical_configuration_digest(
            &config.radii_um,
            config.simulations,
            config.seed,
            config.alpha,
            config.limits,
        )
        .to_string(),
        limits: config.limits,
    };
    let window_summary = window_summary(window.descriptor());
    let maximum_radius_um = *config.radii_um.last().expect("validated radii");
    if pattern.len() < 2 {
        return Ok(ClassicalSpatialResult {
            case_id: pattern.meta.case_id.clone(),
            timepoint: pattern.meta.timepoint.clone(),
            status: ClassicalSpatialStatus::InsufficientPoints,
            window: window_summary,
            geometry: ClassicalGeometrySummary {
                point_count: pattern.len(),
                maximum_radius_um,
                observed_pair_visits: 0,
                total_pair_visits: 0,
                estimated_storage_bytes: observed_plan.estimated_storage_bytes(),
                logical_digest: observed_plan.logical_digest().to_string(),
                execution_mode: "exact".into(),
                duplicate_policy: "reject".into(),
                boundary_distance_owner: "observation_window_2d".into(),
                pair_traversal: "exact_streaming_ordered_pairs".into(),
            },
            configuration,
            null_design,
            curve: unavailable_curve(&config.radii_um),
            inference: None,
            csr_candidate_draws: 0,
        });
    }

    let mut pair_budget = PairVisitBudget::new(config.limits.maximum_pair_visits);
    let observed = observed_plan.border_counts(&config.radii_um, &mut pair_budget)?;
    let observed_pair_visits = pair_budget.used();
    let mut curve = make_curve(
        window.area_um2(),
        pattern.len(),
        &config.radii_um,
        &observed,
    )?;
    let mut simulated_l = Vec::new();
    simulated_l
        .try_reserve_exact(config.simulations)
        .map_err(|_| ClassicalSpatialError::AllocationFailed)?;
    let mut inference_eligible = curve
        .iter()
        .map(|point| point.status == KlPointStatus::Available)
        .collect::<Vec<_>>();
    let mut csr_draws = 0_usize;
    for simulation in 0..config.simulations {
        let seed = derive_seed(config.seed, SeedEndpoint::ClassicalCsr, simulation);
        let (x, y) = sample_conditional_csr(
            window,
            pattern.len(),
            seed,
            &mut csr_draws,
            config.limits.maximum_csr_draws,
        )?;
        let plan = SpatialGeometryPlan2D::new(&x, &y, window, config.limits)?;
        let counts = plan.border_counts(&config.radii_um, &mut pair_budget)?;
        let simulated = make_curve(window.area_um2(), pattern.len(), &config.radii_um, &counts)?;
        let mut row = Vec::with_capacity(simulated.len());
        for (index, point) in simulated.into_iter().enumerate() {
            if let Some(value) = point.l {
                row.push(value);
            } else {
                inference_eligible[index] = false;
                row.push(0.0);
            }
        }
        simulated_l.push(row);
    }

    let inference = if inference_eligible.iter().any(|eligible| *eligible) {
        let observed_l = curve
            .iter()
            .map(|point| point.l.unwrap_or(0.0))
            .collect::<Vec<_>>();
        let envelope = GlobalEnvelope::from_curves_with_eligibility(
            &observed_l,
            &simulated_l,
            config.alpha,
            &inference_eligible,
        )
        .map_err(|error| ClassicalSpatialError::Inference {
            reason: error.to_string(),
        })?;
        for (index, point) in curve.iter_mut().enumerate() {
            point.inference_eligible = inference_eligible[index];
            if inference_eligible[index] {
                point.lower_l = Some(canonical_zero(envelope.lower[index]));
                point.upper_l = Some(canonical_zero(envelope.upper[index]));
            }
        }
        Some(ClassicalInferenceSummary {
            p_global: canonical_zero(envelope.p_global),
            erl_depth: canonical_zero(envelope.erl_depth),
            critical_depth: canonical_zero(envelope.critical_depth),
            simulations: envelope.n_permutations,
            eligible_radius_count: inference_eligible
                .iter()
                .filter(|eligible| **eligible)
                .count(),
        })
    } else {
        None
    };
    let status = if inference.is_some() {
        ClassicalSpatialStatus::Available
    } else {
        ClassicalSpatialStatus::InsufficientInferenceSupport
    };
    Ok(ClassicalSpatialResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        status,
        window: window_summary,
        geometry: ClassicalGeometrySummary {
            point_count: pattern.len(),
            maximum_radius_um,
            observed_pair_visits,
            total_pair_visits: pair_budget.used(),
            estimated_storage_bytes: observed_plan.estimated_storage_bytes(),
            logical_digest: observed_plan.logical_digest().to_string(),
            execution_mode: "exact".into(),
            duplicate_policy: "reject".into(),
            boundary_distance_owner: "observation_window_2d".into(),
            pair_traversal: "exact_streaming_ordered_pairs".into(),
        },
        configuration,
        null_design,
        curve,
        inference,
        csr_candidate_draws: csr_draws,
    })
}

fn validate_pattern_shape(pattern: &Pattern) -> Result<(), ClassicalSpatialError> {
    if pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|value| *value != 1)
    {
        return Err(ClassicalSpatialError::PatternShapeMismatch);
    }
    Ok(())
}

fn make_curve(
    area_um2: f64,
    point_count: usize,
    radii_um: &[f64],
    counts: &BorderCounts,
) -> Result<Vec<HomogeneousKlPoint>, ClassicalSpatialError> {
    let mut curve = Vec::new();
    curve
        .try_reserve_exact(radii_um.len())
        .map_err(|_| ClassicalSpatialError::AllocationFailed)?;
    for (index, radius_um) in radii_um.iter().copied().enumerate() {
        let eligible_centers = counts.eligible_centers[index];
        let ordered_pairs = counts.ordered_pairs[index];
        let theoretical_k = std::f64::consts::PI * radius_um * radius_um;
        if eligible_centers == 0 || point_count == 0 {
            curve.push(HomogeneousKlPoint {
                radius_um,
                status: KlPointStatus::NoEligibleCenters,
                eligible_centers,
                ordered_pairs,
                k: None,
                l: None,
                theoretical_k,
                theoretical_l: radius_um,
                inference_eligible: false,
                lower_l: None,
                upper_l: None,
            });
            continue;
        }
        let denominator = point_count
            .checked_mul(eligible_centers)
            .ok_or(ClassicalSpatialError::SizeOverflow)?;
        let k = area_um2 * ordered_pairs as f64 / denominator as f64;
        let l = (k / std::f64::consts::PI).sqrt();
        if !k.is_finite() || !l.is_finite() {
            return Err(ClassicalSpatialError::Geometry {
                reason: "border K/L produced a non-finite value".into(),
            });
        }
        curve.push(HomogeneousKlPoint {
            radius_um,
            status: KlPointStatus::Available,
            eligible_centers,
            ordered_pairs,
            k: Some(canonical_zero(k)),
            l: Some(canonical_zero(l)),
            theoretical_k,
            theoretical_l: radius_um,
            inference_eligible: false,
            lower_l: None,
            upper_l: None,
        });
    }
    Ok(curve)
}

fn unavailable_curve(radii_um: &[f64]) -> Vec<HomogeneousKlPoint> {
    radii_um
        .iter()
        .copied()
        .map(|radius_um| HomogeneousKlPoint {
            radius_um,
            status: KlPointStatus::NoEligibleCenters,
            eligible_centers: 0,
            ordered_pairs: 0,
            k: None,
            l: None,
            theoretical_k: std::f64::consts::PI * radius_um * radius_um,
            theoretical_l: radius_um,
            inference_eligible: false,
            lower_l: None,
            upper_l: None,
        })
        .collect()
}

pub(crate) fn sample_conditional_csr(
    window: &ObservationWindow2D,
    point_count: usize,
    seed: u64,
    draws: &mut usize,
    maximum_draws: usize,
) -> Result<(Vec<f64>, Vec<f64>), ClassicalSpatialError> {
    let [min_x, min_y, max_x, max_y] = window.bounds_um();
    let width = max_x - min_x;
    let height = max_y - min_y;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(ClassicalSpatialError::Geometry {
            reason: "window bounding box must have positive finite area".into(),
        });
    }
    let mut state = seed;
    let mut x = Vec::new();
    let mut y = Vec::new();
    x.try_reserve_exact(point_count)
        .and_then(|()| y.try_reserve_exact(point_count))
        .map_err(|_| ClassicalSpatialError::AllocationFailed)?;
    let mut unique = BTreeSet::new();
    while x.len() < point_count {
        *draws = draws
            .checked_add(1)
            .ok_or(ClassicalSpatialError::SizeOverflow)?;
        if *draws > maximum_draws {
            return Err(ClassicalSpatialError::CsrDrawLimitExceeded {
                maximum: maximum_draws,
            });
        }
        state = splitmix64(state);
        let x_um = min_x + unit_interval(state) * width;
        state = splitmix64(state);
        let y_um = min_y + unit_interval(state) * height;
        if window.contains(x_um, y_um)
            && unique.insert((canonical_bits(x_um), canonical_bits(y_um)))
        {
            x.push(x_um);
            y.push(y_um);
        }
    }
    Ok((x, y))
}

fn retained_byte_estimate(
    point_count: usize,
    window: &ObservationWindow2D,
    config: &ClassicalSpatialConfig,
) -> Result<usize, ClassicalSpatialError> {
    let plan =
        crate::geom::spatial_index::SpatialIndex2D::estimated_storage_bytes_for_len(point_count)
            .checked_add(
                point_count
                    .checked_mul(std::mem::size_of::<f64>())
                    .ok_or(ClassicalSpatialError::SizeOverflow)?,
            )
            .ok_or(ClassicalSpatialError::SizeOverflow)?;
    let generated = point_count
        .checked_mul(2)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(ClassicalSpatialError::SizeOverflow)?;
    let coordinate_set_work = point_count
        .checked_mul(std::mem::size_of::<((u64, u64), usize)>())
        .and_then(|value| value.checked_mul(4))
        .ok_or(ClassicalSpatialError::SizeOverflow)?;
    let matrix = config
        .simulations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(ClassicalSpatialError::SizeOverflow)?;
    let curve_work = config
        .radii_um
        .len()
        .checked_mul(12)
        .and_then(|value| value.checked_mul(std::mem::size_of::<usize>()))
        .ok_or(ClassicalSpatialError::SizeOverflow)?;
    plan.checked_mul(2)
        .and_then(|value| value.checked_add(window.boundary_storage_bytes()))
        .and_then(|value| value.checked_add(generated))
        .and_then(|value| value.checked_add(coordinate_set_work))
        .and_then(|value| value.checked_add(matrix))
        .and_then(|value| value.checked_add(curve_work))
        .ok_or(ClassicalSpatialError::SizeOverflow)
}

pub(crate) fn window_summary(descriptor: &ObservationWindowDescriptor) -> ClassicalWindowSummary {
    ClassicalWindowSummary {
        coordinate_unit: "micrometre".into(),
        coordinate_frame: "existing_pattern_physical_xy".into(),
        membership_policy: "closed_window".into(),
        area_um2: descriptor.area_um2,
        perimeter_um: descriptor.perimeter_um,
        bounds_um: descriptor.bounds_um,
        component_count: descriptor.component_count,
        hole_count: descriptor.hole_count,
        ring_count: descriptor.ring_count,
        vertex_count: descriptor.vertex_count,
        logical_digest: descriptor.logical_digest.to_string(),
    }
}

fn unit_interval(value: u64) -> f64 {
    (value >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
}

fn canonical_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

pub(super) fn classical_configuration_digest(
    radii_um: &[f64],
    simulations: usize,
    seed: u64,
    alpha: f64,
    limits: ClassicalSpatialLimits,
) -> ContentDigest {
    let mut fields = vec![
        b"marklab-classical-spatial-configuration-v1".to_vec(),
        (radii_um.len() as u128).to_be_bytes().to_vec(),
    ];
    for radius in radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        (simulations as u128).to_be_bytes().to_vec(),
        seed.to_be_bytes().to_vec(),
        alpha.to_bits().to_be_bytes().to_vec(),
        (limits.maximum_points as u128).to_be_bytes().to_vec(),
        (limits.maximum_radii as u128).to_be_bytes().to_vec(),
        (limits.maximum_pair_visits as u128).to_be_bytes().to_vec(),
        (limits.maximum_csr_draws as u128).to_be_bytes().to_vec(),
        (limits.maximum_retained_bytes as u128)
            .to_be_bytes()
            .to_vec(),
    ]);
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

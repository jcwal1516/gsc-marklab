use marklab_workflow::ContentDigest;

use crate::common::finite::canonical_zero;

use crate::{
    classical::{sample_conditional_csr, window_summary, SpatialGeometryPlan2D},
    common::seeds::{derive_seed, SeedEndpoint},
    mark_pair_plan::erl_workspace_bytes,
    permutation::envelopes::GlobalEnvelope,
    ObservationWindow2D, Pattern,
};

use super::epanechnikov_weight;
use super::types::*;

struct Evaluation {
    points: Vec<PairCorrelationPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
    pair_visits: usize,
}

/// Estimate Epanechnikov-smoothed homogeneous g with standard-border CSR inference.
pub fn homogeneous_pair_correlation(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &HomogeneousPairCorrelationConfig,
) -> Result<HomogeneousPairCorrelationResult, HomogeneousPairCorrelationError> {
    if pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|value| *value != 1)
    {
        return Err(HomogeneousPairCorrelationError::PatternShapeMismatch);
    }
    let required_bytes = retained_bytes(pattern.len(), config)?;
    if required_bytes > config.limits.maximum_retained_bytes {
        return Err(HomogeneousPairCorrelationError::RetainedByteLimitExceeded {
            required: required_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, config.limits)
            .map_err(dependency)?;
    let mut used_pair_visits = 0_usize;
    let mut observed = evaluate(
        &observed_plan,
        pattern.len(),
        window.area_um2(),
        config,
        &mut used_pair_visits,
    )?;
    let observed_pair_visits = observed.pair_visits;
    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(config.simulations)
        .map_err(|_| HomogeneousPairCorrelationError::AllocationFailed)?;
    let mut jointly_eligible = observed.eligible.clone();
    let mut csr_candidate_draws = 0_usize;
    for simulation in 0..config.simulations {
        let seed = derive_seed(config.seed, SeedEndpoint::PairCorrelationCsr, simulation);
        let (x, y) = sample_conditional_csr(
            window,
            pattern.len(),
            seed,
            &mut csr_candidate_draws,
            config.limits.maximum_csr_draws,
        )
        .map_err(dependency)?;
        let plan = SpatialGeometryPlan2D::new(&x, &y, window, config.limits).map_err(dependency)?;
        let evaluated = evaluate(
            &plan,
            pattern.len(),
            window.area_um2(),
            config,
            &mut used_pair_visits,
        )?;
        for (joint, current) in jointly_eligible.iter_mut().zip(&evaluated.eligible) {
            *joint &= *current;
        }
        simulated.push(evaluated.values);
    }
    let inference = if jointly_eligible.iter().any(|eligible| *eligible) {
        let envelope = GlobalEnvelope::from_curves_with_eligibility(
            &observed.values,
            &simulated,
            config.alpha,
            &jointly_eligible,
        )
        .map_err(dependency)?;
        for (index, point) in observed.points.iter_mut().enumerate() {
            if jointly_eligible[index] {
                point.inference_eligible = true;
                point.lower_g = Some(canonical_zero(envelope.lower[index]));
                point.upper_g = Some(canonical_zero(envelope.upper[index]));
            }
        }
        Some(HomogeneousPairCorrelationInference {
            p_global: canonical_zero(envelope.p_global),
            erl_depth: canonical_zero(envelope.erl_depth),
            critical_depth: canonical_zero(envelope.critical_depth),
            eligible_radius_count: jointly_eligible.iter().filter(|value| **value).count(),
        })
    } else {
        None
    };
    Ok(HomogeneousPairCorrelationResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        window: window_summary(window.descriptor()),
        kernel: PairCorrelationKernel::Epanechnikov,
        bandwidth_um: config.bandwidth_um,
        edge_correction: "standard_border_radius_plus_bandwidth".into(),
        null_model: "homogeneous_csr_conditional_on_count".into(),
        randomization_unit: "whole_location_pattern".into(),
        geometry_digest: observed_plan.logical_digest().to_string(),
        geometry_build_count: 1,
        observed_pair_visits,
        total_pair_visits: used_pair_visits,
        estimated_storage_bytes: required_bytes,
        configuration_digest: configuration_digest(config).to_string(),
        limits: config.limits,
        simulations: config.simulations,
        seed: config.seed,
        alpha: config.alpha,
        csr_candidate_draws,
        curve: observed.points,
        inference,
    })
}

fn evaluate(
    plan: &SpatialGeometryPlan2D,
    point_count: usize,
    area_um2: f64,
    config: &HomogeneousPairCorrelationConfig,
    used_pair_visits: &mut usize,
) -> Result<Evaluation, HomogeneousPairCorrelationError> {
    let radii = &config.radii_um;
    let mut eligible_centers = vec![0_usize; radii.len()];
    let mut support_pairs = vec![0_usize; radii.len()];
    let mut weight_sums = vec![0.0_f64; radii.len()];
    let maximum_query = radii[radii.len() - 1] + config.bandwidth_um;
    for (source, boundary) in plan.boundary_distances().iter().copied().enumerate() {
        let eligible_end =
            radii.partition_point(|radius| *radius + config.bandwidth_um <= boundary);
        for eligible in &mut eligible_centers[..eligible_end] {
            *eligible = eligible
                .checked_add(1)
                .ok_or(HomogeneousPairCorrelationError::SizeOverflow)?;
        }
        if eligible_end == 0 {
            continue;
        }
        let query_radius = maximum_query.min(boundary);
        let mut visitor_error = None;
        plan.index()
            .visit_within_radius(source, query_radius, |neighbor| {
                if visitor_error.is_some() {
                    return;
                }
                *used_pair_visits = match used_pair_visits.checked_add(1) {
                    Some(value) => value,
                    None => {
                        visitor_error = Some(HomogeneousPairCorrelationError::SizeOverflow);
                        return;
                    }
                };
                if *used_pair_visits > config.limits.maximum_pair_visits {
                    visitor_error = Some(HomogeneousPairCorrelationError::PairVisitLimitExceeded {
                        maximum: config.limits.maximum_pair_visits,
                    });
                    return;
                }
                let lower = neighbor.distance_um - config.bandwidth_um;
                let upper = neighbor.distance_um + config.bandwidth_um;
                let start = radii.partition_point(|radius| *radius <= lower);
                let end = radii
                    .partition_point(|radius| *radius < upper)
                    .min(eligible_end);
                for index in start..end {
                    let weight = epanechnikov_weight(
                        radii[index],
                        neighbor.distance_um,
                        config.bandwidth_um,
                    )
                    .expect("partitioned compact support");
                    if weight < 0.0 || !weight.is_finite() {
                        visitor_error = Some(HomogeneousPairCorrelationError::Dependency(
                            "kernel produced an invalid weight".into(),
                        ));
                        return;
                    }
                    support_pairs[index] = match support_pairs[index].checked_add(1) {
                        Some(value) => value,
                        None => {
                            visitor_error = Some(HomogeneousPairCorrelationError::SizeOverflow);
                            return;
                        }
                    };
                    weight_sums[index] += weight;
                }
            })
            .map_err(dependency)?;
        if let Some(error) = visitor_error {
            return Err(error);
        }
    }
    let mut points = Vec::new();
    let mut values = Vec::new();
    let mut eligible = Vec::new();
    points
        .try_reserve_exact(radii.len())
        .and_then(|()| values.try_reserve_exact(radii.len()))
        .and_then(|()| eligible.try_reserve_exact(radii.len()))
        .map_err(|_| HomogeneousPairCorrelationError::AllocationFailed)?;
    for index in 0..radii.len() {
        let status = if eligible_centers[index] == 0 {
            PairCorrelationPointStatus::NoEligibleCenters
        } else if support_pairs[index] == 0 {
            PairCorrelationPointStatus::NoPairsInKernelSupport
        } else {
            PairCorrelationPointStatus::Available
        };
        let g = (status == PairCorrelationPointStatus::Available)
            .then(|| {
                let denominator = 2.0
                    * std::f64::consts::PI
                    * radii[index]
                    * point_count as f64
                    * eligible_centers[index] as f64;
                area_um2 * weight_sums[index] / denominator
            })
            .filter(|value| value.is_finite())
            .map(canonical_zero);
        if status == PairCorrelationPointStatus::Available && g.is_none() {
            return Err(HomogeneousPairCorrelationError::Dependency(
                "pair-correlation normalization produced a non-finite value".into(),
            ));
        }
        values.push(g.unwrap_or(0.0));
        eligible.push(g.is_some());
        points.push(PairCorrelationPoint {
            radius_um: radii[index],
            status,
            eligible_centers: eligible_centers[index],
            directed_pairs_in_support: support_pairs[index],
            kernel_weight_sum: canonical_zero(weight_sums[index]),
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
        pair_visits: *used_pair_visits,
    })
}

fn retained_bytes(
    point_count: usize,
    config: &HomogeneousPairCorrelationConfig,
) -> Result<usize, HomogeneousPairCorrelationError> {
    let plan =
        crate::geom::spatial_index::SpatialIndex2D::estimated_storage_bytes_for_len(point_count)
            .checked_add(
                point_count
                    .checked_mul(std::mem::size_of::<f64>())
                    .ok_or(HomogeneousPairCorrelationError::SizeOverflow)?,
            )
            .ok_or(HomogeneousPairCorrelationError::SizeOverflow)?;
    let generated = point_count
        .checked_mul(2 * std::mem::size_of::<f64>())
        .ok_or(HomogeneousPairCorrelationError::SizeOverflow)?;
    let matrices = config
        .simulations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(HomogeneousPairCorrelationError::SizeOverflow)?;
    let curve = config
        .radii_um
        .len()
        .checked_mul(
            2 * std::mem::size_of::<PairCorrelationPoint>()
                + 3 * std::mem::size_of::<usize>()
                + 3 * std::mem::size_of::<f64>()
                + std::mem::size_of::<bool>(),
        )
        .ok_or(HomogeneousPairCorrelationError::SizeOverflow)?;
    let erl = erl_workspace_bytes(config.simulations, config.radii_um.len())
        .ok_or(HomogeneousPairCorrelationError::SizeOverflow)?;
    plan.checked_mul(2)
        .and_then(|value| value.checked_add(generated))
        .and_then(|value| value.checked_add(matrices))
        .and_then(|value| value.checked_add(curve))
        .and_then(|value| value.checked_add(erl))
        .ok_or(HomogeneousPairCorrelationError::SizeOverflow)
}

pub(crate) fn configuration_digest(config: &HomogeneousPairCorrelationConfig) -> ContentDigest {
    let mut fields = vec![b"marklab-homogeneous-pair-correlation-config-v1".to_vec()];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
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
        (config.limits.maximum_csr_draws as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_retained_bytes as u128)
            .to_be_bytes()
            .to_vec(),
    ]);
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

fn dependency(error: impl std::fmt::Display) -> HomogeneousPairCorrelationError {
    HomogeneousPairCorrelationError::Dependency(error.to_string())
}

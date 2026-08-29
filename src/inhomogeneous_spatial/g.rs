use marklab_workflow::ContentDigest;

use crate::{
    classical::{window_summary, SpatialGeometryPlan2D},
    common::seeds::{derive_seed, SeedEndpoint},
    pair_correlation::epanechnikov_weight,
    permutation::envelopes::GlobalEnvelope,
    ObservationWindow2D, PairCorrelationKernel, PairCorrelationPointStatus, Pattern,
};

use super::{
    analysis::{canonical_zero, compensated_add, dependency, Counters},
    g_types::{
        InhomogeneousPairCorrelationConfig, InhomogeneousPairCorrelationPoint,
        InhomogeneousPairCorrelationResult,
    },
    identity::{configuration_digest, into_intensity_summary, retained_bytes},
    intensity::{evaluate_fixed_intensities, fit_intensity, sample_fixed_grid_pattern},
    pair::geometry_limits,
    types::{InhomogeneousSpatialError, InhomogeneousSpatialInference},
};

struct Evaluation {
    points: Vec<InhomogeneousPairCorrelationPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
    pair_visits: usize,
}

pub fn analyze_inhomogeneous_pair_correlation(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &InhomogeneousPairCorrelationConfig,
) -> Result<InhomogeneousPairCorrelationResult, InhomogeneousSpatialError> {
    let base = &config.intensity;
    if pattern.len() < 2 {
        return Err(InhomogeneousSpatialError::InsufficientPoints);
    }
    if pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|value| *value != 1)
        || pattern.len() > base.limits.maximum_points
    {
        return Err(InhomogeneousSpatialError::Dependency(
            "pattern shape, validity, or point limit is invalid".into(),
        ));
    }
    let required_bytes = g_retained_bytes(pattern.len(), config)?;
    if required_bytes > base.limits.maximum_retained_bytes {
        return Err(InhomogeneousSpatialError::RetainedByteLimitExceeded {
            required: required_bytes,
            maximum: base.limits.maximum_retained_bytes,
        });
    }
    let mut counters = Counters {
        intensity_evaluations: 0,
        pair_visits: 0,
        null_draws: 0,
    };
    let fitted = fit_intensity(pattern, window, base, &mut counters)?;
    let geometry_limits = geometry_limits(base)?;
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, geometry_limits)
            .map_err(dependency)?;
    let mut observed = evaluate_g(
        &observed_plan,
        &fitted.observed_intensities,
        config,
        &mut counters,
    )?;
    let observed_pair_visits = observed.pair_visits;
    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(base.simulations)
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let mut jointly_eligible = observed.eligible.clone();
    for simulation in 0..base.simulations {
        let seed = derive_seed(
            base.seed,
            SeedEndpoint::InhomogeneousSpatialNull,
            simulation,
        );
        let (x, y) = sample_fixed_grid_pattern(
            window,
            pattern.len(),
            &fitted.grid,
            &fitted.probe_cdf,
            fitted.fixed_grid_total_mass,
            seed,
            base,
            &mut counters,
        )?;
        let intensities =
            evaluate_fixed_intensities(&x, &y, pattern, &fitted, base, &mut counters)?;
        let plan =
            SpatialGeometryPlan2D::new(&x, &y, window, geometry_limits).map_err(dependency)?;
        let evaluated = evaluate_g(&plan, &intensities, config, &mut counters)?;
        for (joint, current) in jointly_eligible.iter_mut().zip(&evaluated.eligible) {
            *joint &= *current;
        }
        simulated.push(evaluated.values);
    }
    let (p_global, erl_depth, critical_depth, eligible_radius_count) =
        if jointly_eligible.iter().any(|value| *value) {
            let envelope = GlobalEnvelope::from_curves_with_eligibility(
                &observed.values,
                &simulated,
                base.alpha,
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
            (
                Some(canonical_zero(envelope.p_global)),
                Some(canonical_zero(envelope.erl_depth)),
                Some(canonical_zero(envelope.critical_depth)),
                jointly_eligible.iter().filter(|value| **value).count(),
            )
        } else {
            (None, None, None, 0)
        };
    let intensity = into_intensity_summary(pattern, window, base, fitted)?;
    Ok(InhomogeneousPairCorrelationResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        window: window_summary(window.descriptor()),
        intensity,
        kernel: PairCorrelationKernel::Epanechnikov,
        pair_bandwidth_um: config.pair_bandwidth_um,
        intensity_bandwidth_um: base.bandwidth_um,
        edge_correction: "standard_border_radius_plus_pair_bandwidth_inverse_intensity_ratio"
            .into(),
        configuration_digest: g_configuration_digest(config).to_string(),
        observed_pair_visits,
        total_pair_visits: counters.pair_visits,
        intensity_evaluations: counters.intensity_evaluations,
        estimated_storage_bytes: required_bytes,
        limits: base.limits,
        curve: observed.points,
        inference: InhomogeneousSpatialInference {
            null_model: "fixed_gridded_inhomogeneous_binomial".into(),
            randomization_unit: "whole_location_pattern_conditioned_on_count".into(),
            p_global,
            erl_depth,
            critical_depth,
            eligible_radius_count,
            simulations_completed: base.simulations,
            seed: base.seed,
            alpha: base.alpha,
            null_draws: counters.null_draws,
        },
    })
}

fn evaluate_g(
    plan: &SpatialGeometryPlan2D,
    intensities: &[f64],
    config: &InhomogeneousPairCorrelationConfig,
    counters: &mut Counters,
) -> Result<Evaluation, InhomogeneousSpatialError> {
    let base = &config.intensity;
    let radii = &base.radii_um;
    let mut centers = zeroed_vec::<usize>(radii.len())?;
    let mut center_inverse_sums = zeroed_vec::<f64>(radii.len())?;
    let mut center_corrections = zeroed_vec::<f64>(radii.len())?;
    let mut pairs = zeroed_vec::<usize>(radii.len())?;
    let mut kernel_sums = zeroed_vec::<f64>(radii.len())?;
    let mut kernel_corrections = zeroed_vec::<f64>(radii.len())?;
    let maximum_query = radii[radii.len() - 1] + config.pair_bandwidth_um;
    let starting_visits = counters.pair_visits;
    for source in 0..intensities.len() {
        let boundary = plan.boundary_distances()[source];
        let eligible_end =
            radii.partition_point(|radius| *radius + config.pair_bandwidth_um <= boundary);
        let inverse_source = 1.0 / intensities[source];
        for index in 0..eligible_end {
            centers[index] = centers[index]
                .checked_add(1)
                .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
            compensated_add(
                &mut center_inverse_sums[index],
                &mut center_corrections[index],
                inverse_source,
            );
        }
        if eligible_end == 0 {
            continue;
        }
        let mut visitor_error = None;
        plan.index()
            .visit_within_radius(source, maximum_query.min(boundary), |neighbor| {
                if visitor_error.is_some() {
                    return;
                }
                if let Err(error) = counters.charge_pair(base.limits.maximum_pair_visits) {
                    visitor_error = Some(error);
                    return;
                }
                let lower = neighbor.distance_um - config.pair_bandwidth_um;
                let upper = neighbor.distance_um + config.pair_bandwidth_um;
                let start = radii.partition_point(|radius| *radius <= lower);
                let end = radii
                    .partition_point(|radius| *radius < upper)
                    .min(eligible_end);
                for index in start..end {
                    let kernel = epanechnikov_weight(
                        radii[index],
                        neighbor.distance_um,
                        config.pair_bandwidth_um,
                    )
                    .expect("partitioned positive kernel support");
                    let weight = kernel * inverse_source / intensities[neighbor.index];
                    if !weight.is_finite() || weight <= 0.0 {
                        visitor_error = Some(InhomogeneousSpatialError::Dependency(
                            "inhomogeneous g pair weight is invalid".into(),
                        ));
                        return;
                    }
                    pairs[index] = match pairs[index].checked_add(1) {
                        Some(value) => value,
                        None => {
                            visitor_error = Some(InhomogeneousSpatialError::SizeOverflow);
                            return;
                        }
                    };
                    compensated_add(
                        &mut kernel_sums[index],
                        &mut kernel_corrections[index],
                        weight,
                    );
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
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    for (index, radius_um) in radii.iter().copied().enumerate() {
        let center_sum = center_inverse_sums[index] + center_corrections[index];
        let kernel_sum = if pairs[index] == 0 {
            0.0
        } else {
            kernel_sums[index] + kernel_corrections[index]
        };
        let status = if centers[index] == 0 {
            PairCorrelationPointStatus::NoEligibleCenters
        } else if pairs[index] == 0 {
            PairCorrelationPointStatus::NoPairsInKernelSupport
        } else {
            PairCorrelationPointStatus::Available
        };
        let g = (status == PairCorrelationPointStatus::Available)
            .then(|| kernel_sum / (2.0 * std::f64::consts::PI * radius_um * center_sum))
            .map(canonical_zero);
        if g.is_some_and(|value| !value.is_finite() || value < 0.0) {
            return Err(InhomogeneousSpatialError::Dependency(
                "inhomogeneous g normalization is invalid".into(),
            ));
        }
        values.push(g.unwrap_or(0.0));
        eligible.push(g.is_some());
        points.push(InhomogeneousPairCorrelationPoint {
            radius_um,
            status,
            eligible_centers: centers[index],
            directed_pairs_in_support: pairs[index],
            inverse_intensity_kernel_sum: canonical_zero(kernel_sum),
            eligible_center_inverse_intensity_sum: canonical_zero(center_sum),
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
        pair_visits: counters.pair_visits - starting_visits,
    })
}

fn g_retained_bytes(
    points: usize,
    config: &InhomogeneousPairCorrelationConfig,
) -> Result<usize, InhomogeneousSpatialError> {
    let base = retained_bytes(points, &config.intensity)?;
    let extra = config
        .intensity
        .radii_um
        .len()
        .checked_mul(
            2 * std::mem::size_of::<InhomogeneousPairCorrelationPoint>()
                + 6 * std::mem::size_of::<f64>()
                + 3 * std::mem::size_of::<usize>()
                + 2 * std::mem::size_of::<bool>(),
        )
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    base.checked_add(extra)
        .ok_or(InhomogeneousSpatialError::SizeOverflow)
}

pub(crate) fn g_configuration_digest(config: &InhomogeneousPairCorrelationConfig) -> ContentDigest {
    ContentDigest::from_framed([
        b"marklab-inhomogeneous-pair-correlation-config-v1".as_slice(),
        configuration_digest(&config.intensity).as_bytes(),
        &config.pair_bandwidth_um.to_bits().to_be_bytes(),
    ])
}

fn zeroed_vec<T: Default + Clone>(length: usize) -> Result<Vec<T>, InhomogeneousSpatialError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    values.resize(length, T::default());
    Ok(values)
}

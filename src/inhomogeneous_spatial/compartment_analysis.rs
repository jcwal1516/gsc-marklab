use crate::{
    classical::{sample_conditional_csr, window_summary, SpatialGeometryPlan2D},
    common::{
        finite::canonical_zero,
        seeds::{derive_seed, SeedEndpoint},
    },
    permutation::envelopes::GlobalEnvelope,
    BinaryCompartmentPartition2D, ClassicalSpatialError, ClassicalSpatialLimits, Pattern,
};

use super::{
    analysis::{dependency, Counters},
    compartment_identity::{
        piecewise_configuration_digest, piecewise_intensity_digest, retained_bytes,
    },
    compartment_types::{
        PiecewiseCompartmentIntensityLevel, PiecewiseCompartmentIntensityPoint,
        PiecewiseCompartmentIntensitySummary, PiecewiseCompartmentRole,
        PiecewiseCompartmentSpatialConfig, PiecewiseCompartmentSpatialResult,
    },
    pair::evaluate_curve,
    InhomogeneousSpatialError, InhomogeneousSpatialInference, InhomogeneousSpatialPoint,
};

pub(super) struct FittedPiecewiseIntensity {
    pub(super) roles: Vec<PiecewiseCompartmentRole>,
    pub(super) intensities: Vec<f64>,
    pub(super) negative_count: usize,
    pub(super) positive_count: usize,
    pub(super) negative_intensity: f64,
    pub(super) positive_intensity: f64,
    pub(super) point_values: Vec<PiecewiseCompartmentIntensityPoint>,
}

pub(super) struct SampledPiecewisePattern {
    pub(super) x: Vec<f64>,
    pub(super) y: Vec<f64>,
    pub(super) intensities: Vec<f64>,
}

struct EnvelopeSummary {
    p_global: Option<f64>,
    erl_depth: Option<f64>,
    critical_depth: Option<f64>,
    eligible_radius_count: usize,
}

/// Estimate leave-one-out binary-compartment intensity and standard-border K/L.
///
/// The location null fixes the observed count in each exact compartment and
/// samples uniformly inside the corresponding polygonal window. Observed
/// events on the shared interface and compartments with fewer than two events
/// are rejected because their leave-one-out event intensity is undefined.
pub fn analyze_piecewise_compartment_spatial_pattern(
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
    config: &PiecewiseCompartmentSpatialConfig,
) -> Result<PiecewiseCompartmentSpatialResult, InhomogeneousSpatialError> {
    if pattern.len() < 2 {
        return Err(InhomogeneousSpatialError::InsufficientPoints);
    }
    if pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|value| *value != 1)
        || pattern.len() > config.limits.maximum_points
    {
        return Err(InhomogeneousSpatialError::Dependency(
            "pattern shape, validity, or point limit is invalid".into(),
        ));
    }
    let required_bytes = retained_bytes(pattern.len(), partition, config)?;
    if required_bytes > config.limits.maximum_retained_bytes {
        return Err(InhomogeneousSpatialError::RetainedByteLimitExceeded {
            required: required_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }
    let mut counters = Counters {
        intensity_evaluations: 0,
        pair_visits: 0,
        null_draws: 0,
    };
    let fitted = fit_piecewise_intensity(pattern, partition, config, &mut counters)?;
    let geometry_limits = ClassicalSpatialLimits::new(
        config.limits.maximum_points,
        config.limits.maximum_radii,
        config.limits.maximum_pair_visits,
        config.limits.maximum_null_draws,
        config.limits.maximum_retained_bytes,
    )
    .map_err(dependency)?;
    let observed_plan = SpatialGeometryPlan2D::new(
        &pattern.x_um,
        &pattern.y_um,
        partition.observation_window(),
        geometry_limits,
    )
    .map_err(dependency)?;
    let mut observed = evaluate_curve(
        &observed_plan,
        &fitted.intensities,
        &config.radii_um,
        config.limits.maximum_pair_visits,
        &mut counters,
    )?;
    let observed_pair_visits = observed.pair_visits;
    drop(observed_plan);

    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(config.simulations)
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let mut jointly_eligible = observed.eligible.clone();
    for simulation in 0..config.simulations {
        let sampled = sample_piecewise_null_pattern(
            pattern.len(),
            partition,
            &fitted,
            config,
            simulation,
            &mut counters,
        )?;
        let plan = SpatialGeometryPlan2D::new(
            &sampled.x,
            &sampled.y,
            partition.observation_window(),
            geometry_limits,
        )
        .map_err(dependency)?;
        let evaluated = evaluate_curve(
            &plan,
            &sampled.intensities,
            &config.radii_um,
            config.limits.maximum_pair_visits,
            &mut counters,
        )?;
        for (joint, current) in jointly_eligible.iter_mut().zip(&evaluated.eligible) {
            *joint &= *current;
        }
        simulated.push(evaluated.values);
    }

    let envelope = attach_envelope(
        &mut observed.points,
        &observed.values,
        &simulated,
        &jointly_eligible,
        config,
    )?;
    let compartment_queries = fitted.roles.len();
    let intensity = into_piecewise_intensity_summary(pattern, partition, fitted);
    Ok(PiecewiseCompartmentSpatialResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        window: window_summary(partition.observation_window().descriptor()),
        intensity,
        edge_correction: "standard_border_inverse_intensity_ratio".into(),
        configuration_digest: piecewise_configuration_digest(config).to_string(),
        compartment_queries,
        observed_pair_visits,
        total_pair_visits: counters.pair_visits,
        estimated_storage_bytes: required_bytes,
        limits: config.limits,
        curve: observed.points,
        inference: InhomogeneousSpatialInference {
            null_model: "fixed_binary_compartment_counts_uniform_within_exact_partition".into(),
            randomization_unit: "whole_location_pattern_conditioned_on_binary_compartment_counts"
                .into(),
            p_global: envelope.p_global,
            erl_depth: envelope.erl_depth,
            critical_depth: envelope.critical_depth,
            eligible_radius_count: envelope.eligible_radius_count,
            simulations_completed: config.simulations,
            seed: config.seed,
            alpha: config.alpha,
            null_draws: counters.null_draws,
        },
    })
}

pub(super) fn into_piecewise_intensity_summary(
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
    fitted: FittedPiecewiseIntensity,
) -> PiecewiseCompartmentIntensitySummary {
    let descriptor = partition.descriptor();
    let artifact_digest = piecewise_intensity_digest(
        pattern,
        partition,
        fitted.negative_count,
        fitted.positive_count,
        fitted.negative_intensity,
        fitted.positive_intensity,
        &fitted.point_values,
    );
    PiecewiseCompartmentIntensitySummary {
        estimator: "piecewise_constant_binary_compartment".into(),
        cross_fit: "leave_one_out_within_compartment".into(),
        boundary_correction: "exact_compartment_area".into(),
        interface_event_policy: "reject".into(),
        partition_digest: descriptor.logical_digest.to_string(),
        artifact_digest: artifact_digest.to_string(),
        negative: PiecewiseCompartmentIntensityLevel {
            role: PiecewiseCompartmentRole::Negative,
            compartment_id: descriptor.negative_compartment_id.clone(),
            area_um2: descriptor.negative_area_um2,
            event_count: fitted.negative_count,
            leave_one_out_training_count: fitted.negative_count - 1,
            intensity_per_um2: fitted.negative_intensity,
        },
        positive: PiecewiseCompartmentIntensityLevel {
            role: PiecewiseCompartmentRole::Positive,
            compartment_id: descriptor.positive_compartment_id.clone(),
            area_um2: descriptor.positive_area_um2,
            event_count: fitted.positive_count,
            leave_one_out_training_count: fitted.positive_count - 1,
            intensity_per_um2: fitted.positive_intensity,
        },
        point_values: fitted.point_values,
    }
}

pub(super) fn fit_piecewise_intensity(
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
    config: &PiecewiseCompartmentSpatialConfig,
    counters: &mut Counters,
) -> Result<FittedPiecewiseIntensity, InhomogeneousSpatialError> {
    let mut roles = Vec::new();
    roles
        .try_reserve_exact(pattern.len())
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let mut negative_count = 0_usize;
    let mut positive_count = 0_usize;
    for row in 0..pattern.len() {
        counters.intensity_evaluations = counters
            .intensity_evaluations
            .checked_add(1)
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
        if counters.intensity_evaluations > config.limits.maximum_compartment_queries {
            return Err(InhomogeneousSpatialError::CompartmentQueryLimitExceeded {
                maximum: config.limits.maximum_compartment_queries,
            });
        }
        let signed = partition
            .signed_interface_distance_um(pattern.x_um[row], pattern.y_um[row])
            .map_err(dependency)?;
        let role = if signed < 0.0 {
            negative_count = negative_count
                .checked_add(1)
                .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
            PiecewiseCompartmentRole::Negative
        } else if signed > 0.0 {
            positive_count = positive_count
                .checked_add(1)
                .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
            PiecewiseCompartmentRole::Positive
        } else {
            return Err(InhomogeneousSpatialError::PointOnCompartmentInterface { row });
        };
        roles.push(role);
    }
    let descriptor = partition.descriptor();
    require_two(&descriptor.negative_compartment_id, negative_count)?;
    require_two(&descriptor.positive_compartment_id, positive_count)?;
    let negative_intensity = (negative_count - 1) as f64 / descriptor.negative_area_um2;
    let positive_intensity = (positive_count - 1) as f64 / descriptor.positive_area_um2;
    if !negative_intensity.is_finite()
        || negative_intensity <= 0.0
        || !positive_intensity.is_finite()
        || positive_intensity <= 0.0
    {
        return Err(InhomogeneousSpatialError::Dependency(
            "piecewise compartment intensity is not finite and positive".into(),
        ));
    }
    let mut intensities = Vec::new();
    let mut point_values = Vec::new();
    intensities
        .try_reserve_exact(pattern.len())
        .and_then(|()| point_values.try_reserve_exact(pattern.len()))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    for (row, role) in roles.iter().copied().enumerate() {
        let (compartment_id, observed_count, intensity) = match role {
            PiecewiseCompartmentRole::Negative => (
                descriptor.negative_compartment_id.clone(),
                negative_count,
                negative_intensity,
            ),
            PiecewiseCompartmentRole::Positive => (
                descriptor.positive_compartment_id.clone(),
                positive_count,
                positive_intensity,
            ),
        };
        intensities.push(intensity);
        point_values.push(PiecewiseCompartmentIntensityPoint {
            row,
            role,
            compartment_id,
            observed_compartment_count: observed_count,
            training_point_count: observed_count - 1,
            intensity_per_um2: intensity,
        });
    }
    Ok(FittedPiecewiseIntensity {
        roles,
        intensities,
        negative_count,
        positive_count,
        negative_intensity,
        positive_intensity,
        point_values,
    })
}

fn require_two(compartment_id: &str, observed: usize) -> Result<(), InhomogeneousSpatialError> {
    if observed < 2 {
        return Err(InhomogeneousSpatialError::SparseCompartment {
            compartment_id: compartment_id.into(),
            observed,
        });
    }
    Ok(())
}

pub(super) fn sample_compartment(
    window: &crate::ObservationWindow2D,
    point_count: usize,
    seed: u64,
    draws: &mut usize,
    maximum_draws: usize,
) -> Result<(Vec<f64>, Vec<f64>), InhomogeneousSpatialError> {
    match sample_conditional_csr(window, point_count, seed, draws, maximum_draws) {
        Ok(points) => Ok(points),
        Err(ClassicalSpatialError::CsrDrawLimitExceeded { maximum }) => {
            Err(InhomogeneousSpatialError::NullDrawLimitExceeded { maximum })
        }
        Err(error) => Err(dependency(error)),
    }
}

pub(super) fn sample_piecewise_null_pattern(
    point_count: usize,
    partition: &BinaryCompartmentPartition2D,
    fitted: &FittedPiecewiseIntensity,
    config: &PiecewiseCompartmentSpatialConfig,
    simulation: usize,
    counters: &mut Counters,
) -> Result<SampledPiecewisePattern, InhomogeneousSpatialError> {
    let seed_index = simulation
        .checked_mul(2)
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let negative_seed = derive_seed(
        config.seed,
        SeedEndpoint::PiecewiseCompartmentSpatialNull,
        seed_index,
    );
    let positive_seed = derive_seed(
        config.seed,
        SeedEndpoint::PiecewiseCompartmentSpatialNull,
        seed_index
            .checked_add(1)
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?,
    );
    let (negative_x, negative_y) = sample_compartment(
        partition.negative_window(),
        fitted.negative_count,
        negative_seed,
        &mut counters.null_draws,
        config.limits.maximum_null_draws,
    )?;
    let (positive_x, positive_y) = sample_compartment(
        partition.positive_window(),
        fitted.positive_count,
        positive_seed,
        &mut counters.null_draws,
        config.limits.maximum_null_draws,
    )?;
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut intensities = Vec::new();
    x.try_reserve_exact(point_count)
        .and_then(|()| y.try_reserve_exact(point_count))
        .and_then(|()| intensities.try_reserve_exact(point_count))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    x.extend(negative_x);
    x.extend(positive_x);
    y.extend(negative_y);
    y.extend(positive_y);
    intensities.resize(fitted.negative_count, fitted.negative_intensity);
    intensities.resize(point_count, fitted.positive_intensity);
    Ok(SampledPiecewisePattern { x, y, intensities })
}

fn attach_envelope(
    points: &mut [InhomogeneousSpatialPoint],
    observed: &[f64],
    simulated: &[Vec<f64>],
    jointly_eligible: &[bool],
    config: &PiecewiseCompartmentSpatialConfig,
) -> Result<EnvelopeSummary, InhomogeneousSpatialError> {
    if !jointly_eligible.iter().any(|value| *value) {
        return Ok(EnvelopeSummary {
            p_global: None,
            erl_depth: None,
            critical_depth: None,
            eligible_radius_count: 0,
        });
    }
    let envelope = GlobalEnvelope::from_curves_with_eligibility(
        observed,
        simulated,
        config.alpha,
        jointly_eligible,
    )
    .map_err(dependency)?;
    for (index, point) in points.iter_mut().enumerate() {
        if jointly_eligible[index] {
            point.inference_eligible = true;
            point.lower_l = Some(canonical_zero(envelope.lower[index]));
            point.upper_l = Some(canonical_zero(envelope.upper[index]));
        }
    }
    Ok(EnvelopeSummary {
        p_global: Some(canonical_zero(envelope.p_global)),
        erl_depth: Some(canonical_zero(envelope.erl_depth)),
        critical_depth: Some(canonical_zero(envelope.critical_depth)),
        eligible_radius_count: jointly_eligible.iter().filter(|value| **value).count(),
    })
}

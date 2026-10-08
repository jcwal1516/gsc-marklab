use crate::{
    classical::{window_summary, SpatialGeometryPlan2D},
    common::finite::canonical_zero,
    permutation::envelopes::GlobalEnvelope,
    BinaryCompartmentPartition2D, ClassicalSpatialLimits, PairCorrelationKernel, Pattern,
};

use super::{
    analysis::{dependency, Counters},
    compartment_analysis::{
        fit_piecewise_intensity, into_piecewise_intensity_summary, sample_piecewise_null_pattern,
    },
    compartment_identity::{piecewise_g_configuration_digest, piecewise_g_retained_bytes},
    g::evaluate_g,
    InhomogeneousPairCorrelationPoint, InhomogeneousSpatialError, InhomogeneousSpatialInference,
    PiecewiseCompartmentPairCorrelationConfig, PiecewiseCompartmentPairCorrelationResult,
};

struct EnvelopeSummary {
    p_global: Option<f64>,
    erl_depth: Option<f64>,
    critical_depth: Option<f64>,
    eligible_radius_count: usize,
}

/// Estimate piecewise-compartment intensity-reweighted Epanechnikov pair correlation.
///
/// This uses the same leave-one-out intensity rows and fixed-compartment-count
/// exact-window null as [`super::analyze_piecewise_compartment_spatial_pattern`].
pub fn analyze_piecewise_compartment_pair_correlation(
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
    config: &PiecewiseCompartmentPairCorrelationConfig,
) -> Result<PiecewiseCompartmentPairCorrelationResult, InhomogeneousSpatialError> {
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
    let required_bytes = piecewise_g_retained_bytes(pattern.len(), partition, config)?;
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
    let fitted = fit_piecewise_intensity(pattern, partition, base, &mut counters)?;
    let geometry_limits = ClassicalSpatialLimits::new(
        base.limits.maximum_points,
        base.limits.maximum_radii,
        base.limits.maximum_pair_visits,
        base.limits.maximum_null_draws,
        base.limits.maximum_retained_bytes,
    )
    .map_err(dependency)?;
    let observed_plan = SpatialGeometryPlan2D::new(
        &pattern.x_um,
        &pattern.y_um,
        partition.observation_window(),
        geometry_limits,
    )
    .map_err(dependency)?;
    let mut observed = evaluate_g(
        &observed_plan,
        &fitted.intensities,
        &base.radii_um,
        config.pair_bandwidth_um,
        base.limits.maximum_pair_visits,
        &mut counters,
    )?;
    let observed_pair_visits = observed.pair_visits;
    drop(observed_plan);

    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(base.simulations)
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let mut jointly_eligible = observed.eligible.clone();
    for simulation in 0..base.simulations {
        let sampled = sample_piecewise_null_pattern(
            pattern.len(),
            partition,
            &fitted,
            base,
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
        let evaluated = evaluate_g(
            &plan,
            &sampled.intensities,
            &base.radii_um,
            config.pair_bandwidth_um,
            base.limits.maximum_pair_visits,
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
    Ok(PiecewiseCompartmentPairCorrelationResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        window: window_summary(partition.observation_window().descriptor()),
        intensity,
        kernel: PairCorrelationKernel::Epanechnikov,
        pair_bandwidth_um: config.pair_bandwidth_um,
        edge_correction: "standard_border_radius_plus_pair_bandwidth_inverse_intensity_ratio"
            .into(),
        configuration_digest: piecewise_g_configuration_digest(config).to_string(),
        compartment_queries,
        observed_pair_visits,
        total_pair_visits: counters.pair_visits,
        estimated_storage_bytes: required_bytes,
        limits: base.limits,
        curve: observed.points,
        inference: InhomogeneousSpatialInference {
            null_model: "fixed_binary_compartment_counts_uniform_within_exact_partition".into(),
            randomization_unit: "whole_location_pattern_conditioned_on_binary_compartment_counts"
                .into(),
            p_global: envelope.p_global,
            erl_depth: envelope.erl_depth,
            critical_depth: envelope.critical_depth,
            eligible_radius_count: envelope.eligible_radius_count,
            simulations_completed: base.simulations,
            seed: base.seed,
            alpha: base.alpha,
            null_draws: counters.null_draws,
        },
    })
}

fn attach_envelope(
    points: &mut [InhomogeneousPairCorrelationPoint],
    observed: &[f64],
    simulated: &[Vec<f64>],
    jointly_eligible: &[bool],
    config: &PiecewiseCompartmentPairCorrelationConfig,
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
        config.intensity.alpha,
        jointly_eligible,
    )
    .map_err(dependency)?;
    for (index, point) in points.iter_mut().enumerate() {
        if jointly_eligible[index] {
            point.inference_eligible = true;
            point.lower_g = Some(canonical_zero(envelope.lower[index]));
            point.upper_g = Some(canonical_zero(envelope.upper[index]));
        }
    }
    Ok(EnvelopeSummary {
        p_global: Some(canonical_zero(envelope.p_global)),
        erl_depth: Some(canonical_zero(envelope.erl_depth)),
        critical_depth: Some(canonical_zero(envelope.critical_depth)),
        eligible_radius_count: jointly_eligible.iter().filter(|value| **value).count(),
    })
}

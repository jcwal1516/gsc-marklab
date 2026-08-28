use crate::{
    classical::{window_summary, SpatialGeometryPlan2D},
    common::seeds::{derive_seed, SeedEndpoint},
    permutation::envelopes::GlobalEnvelope,
    ObservationWindow2D, Pattern,
};

use super::{
    identity::{configuration_digest, into_intensity_summary, retained_bytes},
    intensity::{evaluate_fixed_intensities, fit_intensity, sample_fixed_grid_pattern},
    pair::{evaluate_curve, geometry_limits},
    types::*,
};

pub(super) struct Counters {
    pub(super) intensity_evaluations: usize,
    pub(super) pair_visits: usize,
    pub(super) null_draws: usize,
}

pub fn analyze_inhomogeneous_spatial_pattern(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &InhomogeneousSpatialConfig,
) -> Result<InhomogeneousSpatialResult, InhomogeneousSpatialError> {
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
    let required_bytes = retained_bytes(pattern.len(), config)?;
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
    let fitted = fit_intensity(pattern, window, config, &mut counters)?;

    let geometry_limits = geometry_limits(config)?;
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, geometry_limits)
            .map_err(dependency)?;
    let mut observed = evaluate_curve(
        &observed_plan,
        &fitted.observed_intensities,
        config,
        &mut counters,
    )?;
    let observed_pair_visits = observed.pair_visits;

    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(config.simulations)
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let mut jointly_eligible = observed.eligible.clone();
    for simulation in 0..config.simulations {
        let seed = derive_seed(
            config.seed,
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
            config,
            &mut counters,
        )?;
        let intensities =
            evaluate_fixed_intensities(&x, &y, pattern, &fitted, config, &mut counters)?;
        let plan =
            SpatialGeometryPlan2D::new(&x, &y, window, geometry_limits).map_err(dependency)?;
        let evaluated = evaluate_curve(&plan, &intensities, config, &mut counters)?;
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
                config.alpha,
                &jointly_eligible,
            )
            .map_err(dependency)?;
            for (index, point) in observed.points.iter_mut().enumerate() {
                if jointly_eligible[index] {
                    point.inference_eligible = true;
                    point.lower_l = Some(canonical_zero(envelope.lower[index]));
                    point.upper_l = Some(canonical_zero(envelope.upper[index]));
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
    let intensity = into_intensity_summary(pattern, window, config, fitted)?;
    Ok(InhomogeneousSpatialResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        window: window_summary(window.descriptor()),
        intensity,
        edge_correction: "standard_border_inverse_intensity_ratio".into(),
        configuration_digest: configuration_digest(config).to_string(),
        observed_pair_visits,
        total_pair_visits: counters.pair_visits,
        intensity_evaluations: counters.intensity_evaluations,
        estimated_storage_bytes: required_bytes,
        limits: config.limits,
        curve: observed.points,
        inference: InhomogeneousSpatialInference {
            null_model: "fixed_gridded_inhomogeneous_binomial".into(),
            randomization_unit: "whole_location_pattern_conditioned_on_count".into(),
            p_global,
            erl_depth,
            critical_depth,
            eligible_radius_count,
            simulations_completed: config.simulations,
            seed: config.seed,
            alpha: config.alpha,
            null_draws: counters.null_draws,
        },
    })
}

impl Counters {
    pub(super) fn charge_intensity(
        &mut self,
        config: &InhomogeneousSpatialConfig,
    ) -> Result<(), InhomogeneousSpatialError> {
        self.intensity_evaluations = self
            .intensity_evaluations
            .checked_add(1)
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
        if self.intensity_evaluations > config.limits.maximum_intensity_evaluations {
            return Err(
                InhomogeneousSpatialError::IntensityEvaluationLimitExceeded {
                    maximum: config.limits.maximum_intensity_evaluations,
                },
            );
        }
        Ok(())
    }

    pub(super) fn charge_pair(
        &mut self,
        config: &InhomogeneousSpatialConfig,
    ) -> Result<(), InhomogeneousSpatialError> {
        self.pair_visits = self
            .pair_visits
            .checked_add(1)
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
        if self.pair_visits > config.limits.maximum_pair_visits {
            return Err(InhomogeneousSpatialError::PairVisitLimitExceeded {
                maximum: config.limits.maximum_pair_visits,
            });
        }
        Ok(())
    }

    pub(super) fn charge_null_draw(
        &mut self,
        config: &InhomogeneousSpatialConfig,
    ) -> Result<(), InhomogeneousSpatialError> {
        self.null_draws = self
            .null_draws
            .checked_add(1)
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
        if self.null_draws > config.limits.maximum_null_draws {
            return Err(InhomogeneousSpatialError::NullDrawLimitExceeded {
                maximum: config.limits.maximum_null_draws,
            });
        }
        Ok(())
    }
}

pub(super) fn compensated_add(sum: &mut f64, correction: &mut f64, value: f64) {
    let corrected = value - *correction;
    let next = *sum + corrected;
    *correction = (next - *sum) - corrected;
    *sum = next;
}

pub(super) fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

pub(super) fn dependency(error: impl std::fmt::Display) -> InhomogeneousSpatialError {
    InhomogeneousSpatialError::Dependency(error.to_string())
}

use crate::{
    classical::{window_summary, SpatialGeometryPlan2D},
    common::seeds::{derive_seed, SeedEndpoint},
    permutation::envelopes::GlobalEnvelope,
    ObservationWindow2D, Pattern,
};

use super::{
    identity::{configuration_digest, fixed_grid_digest, intensity_result_digest, retained_bytes},
    intensity::{
        boundary_mass, build_grid, extrema, fixed_probe_cdf, kernel_sum, sample_fixed_grid_pattern,
        validate_intensity,
    },
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
    let grid = build_grid(window, config)?;
    let mut counters = Counters {
        intensity_evaluations: 0,
        pair_visits: 0,
        null_draws: 0,
    };
    let mut point_values = Vec::new();
    let mut observed_intensities = Vec::new();
    point_values
        .try_reserve_exact(pattern.len())
        .and_then(|()| observed_intensities.try_reserve_exact(pattern.len()))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let finite_scale = pattern.len() as f64 / (pattern.len() - 1) as f64;
    for row in 0..pattern.len() {
        let location = (pattern.x_um[row], pattern.y_um[row]);
        let boundary_mass = boundary_mass(location, &grid, config, &mut counters)?;
        let raw = kernel_sum(
            location,
            &pattern.x_um,
            &pattern.y_um,
            Some(row),
            config,
            &mut counters,
        )?;
        let intensity = finite_scale * raw / boundary_mass;
        validate_intensity(row, intensity, config.minimum_intensity_per_um2)?;
        point_values.push(InhomogeneousIntensityPoint {
            row,
            intensity_per_um2: canonical_zero(intensity),
            boundary_mass,
            training_point_count: pattern.len() - 1,
        });
        observed_intensities.push(intensity);
    }

    let geometry_limits = geometry_limits(config)?;
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, geometry_limits)
            .map_err(dependency)?;
    let mut observed =
        evaluate_curve(&observed_plan, &observed_intensities, config, &mut counters)?;
    let observed_pair_visits = observed.pair_visits;

    let (probe_cdf, cdf_total, fixed_grid) =
        fixed_probe_cdf(pattern, &grid, config, &mut counters)?;
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
            &grid,
            &probe_cdf,
            cdf_total,
            seed,
            config,
            &mut counters,
        )?;
        let mut intensities = Vec::new();
        intensities
            .try_reserve_exact(pattern.len())
            .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
        for row in 0..pattern.len() {
            let location = (x[row], y[row]);
            let correction = boundary_mass(location, &grid, config, &mut counters)?;
            let intensity = kernel_sum(
                location,
                &pattern.x_um,
                &pattern.y_um,
                None,
                config,
                &mut counters,
            )? / correction;
            validate_intensity(row, intensity, config.minimum_intensity_per_um2)?;
            intensities.push(intensity);
        }
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
    let (minimum_intensity, maximum_intensity) = extrema(&observed_intensities)?;
    let fixed_grid_digest = fixed_grid_digest(window, config, &fixed_grid);
    let artifact_digest =
        intensity_result_digest(pattern, window, config, &point_values, &fixed_grid);
    Ok(InhomogeneousSpatialResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        window: window_summary(window.descriptor()),
        intensity: InhomogeneousIntensitySummary {
            estimator: "gaussian_kernel".into(),
            kernel: "isotropic_gaussian_2d".into(),
            cross_fit: "leave_one_out_n_over_n_minus_one".into(),
            boundary_correction: "deterministic_cell_center_quadrature".into(),
            bandwidth_um: config.bandwidth_um,
            integration_grid: config.integration_grid,
            retained_probe_count: grid.probes.len(),
            probe_spacing_um: grid.spacing_um,
            maximum_probe_displacement_um: 0.5 * grid.spacing_um[0].hypot(grid.spacing_um[1]),
            minimum_intensity_per_um2: config.minimum_intensity_per_um2,
            observed_minimum_intensity_per_um2: minimum_intensity,
            observed_maximum_intensity_per_um2: maximum_intensity,
            artifact_digest: artifact_digest.to_string(),
            point_values,
            fixed_grid_digest: fixed_grid_digest.to_string(),
            fixed_grid_total_mass: cdf_total,
            fixed_grid,
        },
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

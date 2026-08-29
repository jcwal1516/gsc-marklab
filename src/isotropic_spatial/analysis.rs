use marklab_workflow::ContentDigest;

use crate::{
    classical::{
        sample_conditional_csr, window_summary, ClassicalInferenceSummary, ClassicalNullDesign,
        ClassicalNullModel, ClassicalRandomizationUnit, ClassicalSpatialLimits,
        SpatialGeometryPlan2D,
    },
    common::seeds::{derive_seed, SeedEndpoint},
    data::Pattern,
    geom::window::{ObservationWindow2D, ObservationWindowError},
    permutation::envelopes::GlobalEnvelope,
};

use super::types::*;

/// Compute exact homogeneous isotropic K/L with conditional-CSR inference.
pub fn analyze_isotropic_spatial_pattern(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &IsotropicSpatialConfig,
) -> Result<IsotropicSpatialResult, IsotropicSpatialError> {
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
    .map_err(|error| IsotropicSpatialError::InvalidPattern {
        reason: error.to_string(),
    })?;
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, classical_limits)
            .map_err(|error| IsotropicSpatialError::InvalidPattern {
                reason: error.to_string(),
            })?;
    let maximum_radius_um = *config.radii_um.last().expect("validated radii");
    let null_design = ClassicalNullDesign {
        null_model: ClassicalNullModel::HomogeneousCsrConditionalOnCount,
        randomization_unit: ClassicalRandomizationUnit::WholeLocationPattern,
        conditioned_point_count: pattern.len(),
        simulations: config.simulations,
        seed: config.seed,
        alpha: config.alpha,
    };
    let configuration = IsotropicConfigurationSummary {
        radius_count: config.radii_um.len(),
        logical_digest: isotropic_configuration_digest(config).to_string(),
        limits: config.limits,
    };
    let mut geometry = IsotropicGeometrySummary {
        point_count: pattern.len(),
        maximum_radius_um,
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
        pair_traversal: "exact_streaming_unordered_pairs_with_two_directed_arc_weights".into(),
    };
    if pattern.len() < 2 {
        return Ok(IsotropicSpatialResult {
            case_id: pattern.meta.case_id.clone(),
            timepoint: pattern.meta.timepoint.clone(),
            correction: "isotropic".into(),
            status: IsotropicSpatialStatus::InsufficientPoints,
            window: window_summary(window.descriptor()),
            geometry,
            configuration,
            null_design,
            curve: unavailable_curve(&config.radii_um),
            inference: None,
            csr_candidate_draws: 0,
        });
    }

    let mut budget = IsotropicWorkBudget::new(config.limits, window);
    let observed = isotropic_curve(
        &pattern.x_um,
        &pattern.y_um,
        window,
        &config.radii_um,
        &mut budget,
    )?;
    geometry.observed_pair_visits = budget.pair_visits;
    geometry.observed_visible_arc_evaluations = budget.visible_arc_evaluations;
    let mut curve = make_curve(
        window.area_um2(),
        pattern.len(),
        &config.radii_um,
        &observed,
    )?;
    let mut simulated_l = Vec::new();
    simulated_l
        .try_reserve_exact(config.simulations)
        .map_err(|_| IsotropicSpatialError::AllocationFailed)?;
    let mut csr_draws = 0_usize;
    for simulation in 0..config.simulations {
        let seed = derive_seed(config.seed, SeedEndpoint::IsotropicSpatialCsr, simulation);
        let (x, y) = sample_conditional_csr(
            window,
            pattern.len(),
            seed,
            &mut csr_draws,
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
        let work = isotropic_curve(&x, &y, window, &config.radii_um, &mut budget)?;
        let simulated = make_curve(window.area_um2(), pattern.len(), &config.radii_um, &work)?;
        simulated_l.push(
            simulated
                .into_iter()
                .map(|point| point.l.expect("two-point isotropic curve is available"))
                .collect(),
        );
    }
    let observed_l = curve
        .iter()
        .map(|point| point.l.expect("two-point isotropic curve is available"))
        .collect::<Vec<_>>();
    let eligibility = vec![true; observed_l.len()];
    let envelope = GlobalEnvelope::from_curves_with_eligibility(
        &observed_l,
        &simulated_l,
        config.alpha,
        &eligibility,
    )
    .map_err(|error| IsotropicSpatialError::Inference {
        reason: error.to_string(),
    })?;
    for (index, point) in curve.iter_mut().enumerate() {
        point.lower_l = Some(canonical_zero(envelope.lower[index]));
        point.upper_l = Some(canonical_zero(envelope.upper[index]));
    }
    geometry.total_pair_visits = budget.pair_visits;
    geometry.total_visible_arc_evaluations = budget.visible_arc_evaluations;
    geometry.total_arc_segment_tests = budget.arc_segment_tests;
    geometry.total_arc_membership_queries = budget.arc_membership_queries;
    geometry.maximum_boundary_intersection_angles = budget.maximum_intersection_angles;
    Ok(IsotropicSpatialResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        correction: "isotropic".into(),
        status: IsotropicSpatialStatus::Available,
        window: window_summary(window.descriptor()),
        geometry,
        configuration,
        null_design,
        curve,
        inference: Some(ClassicalInferenceSummary {
            p_global: canonical_zero(envelope.p_global),
            erl_depth: canonical_zero(envelope.erl_depth),
            critical_depth: canonical_zero(envelope.critical_depth),
            simulations: envelope.n_permutations,
            eligible_radius_count: config.radii_um.len(),
        }),
        csr_candidate_draws: csr_draws,
    })
}

struct IsotropicCurveWork {
    directed_pairs: Vec<usize>,
    visible_arc_evaluations: Vec<usize>,
    visible_fraction_sums: Vec<f64>,
    inverse_visible_fraction_sums: Vec<f64>,
}

fn isotropic_curve(
    x: &[f64],
    y: &[f64],
    window: &ObservationWindow2D,
    radii_um: &[f64],
    budget: &mut IsotropicWorkBudget,
) -> Result<IsotropicCurveWork, IsotropicSpatialError> {
    let mut directed_pair_starts = vec![0_usize; radii_um.len()];
    let mut evaluation_starts = vec![0_usize; radii_um.len()];
    let mut fraction_starts = vec![0.0_f64; radii_um.len()];
    let mut inverse_starts = vec![0.0_f64; radii_um.len()];
    let maximum_radius = *radii_um.last().expect("validated radii");
    for left in 0..x.len() {
        for right in (left + 1)..x.len() {
            budget.charge_pair()?;
            let displacement_x = x[right] - x[left];
            let displacement_y = y[right] - y[left];
            let distance = displacement_x.hypot(displacement_y);
            if distance > maximum_radius {
                continue;
            }
            let start = radii_um.partition_point(|radius| *radius < distance);
            if start == radii_um.len() {
                continue;
            }
            let left_fraction =
                budget.visible_fraction(window, x[left], y[left], distance, left, right)?;
            let right_fraction =
                budget.visible_fraction(window, x[right], y[right], distance, right, left)?;
            directed_pair_starts[start] = directed_pair_starts[start]
                .checked_add(2)
                .ok_or(IsotropicSpatialError::SizeOverflow)?;
            evaluation_starts[start] = evaluation_starts[start]
                .checked_add(2)
                .ok_or(IsotropicSpatialError::SizeOverflow)?;
            fraction_starts[start] += left_fraction + right_fraction;
            inverse_starts[start] += 1.0 / left_fraction + 1.0 / right_fraction;
            if !fraction_starts[start].is_finite() || !inverse_starts[start].is_finite() {
                return Err(IsotropicSpatialError::InvalidPattern {
                    reason: "isotropic pair accumulation is non-finite".into(),
                });
            }
        }
    }
    cumulative_counts(&mut directed_pair_starts)?;
    cumulative_counts(&mut evaluation_starts)?;
    cumulative_finite(&mut fraction_starts)?;
    cumulative_finite(&mut inverse_starts)?;
    Ok(IsotropicCurveWork {
        directed_pairs: directed_pair_starts,
        visible_arc_evaluations: evaluation_starts,
        visible_fraction_sums: fraction_starts,
        inverse_visible_fraction_sums: inverse_starts,
    })
}

fn make_curve(
    area_um2: f64,
    point_count: usize,
    radii_um: &[f64],
    work: &IsotropicCurveWork,
) -> Result<Vec<IsotropicKlPoint>, IsotropicSpatialError> {
    let denominator = point_count
        .checked_mul(point_count - 1)
        .ok_or(IsotropicSpatialError::SizeOverflow)? as f64;
    radii_um
        .iter()
        .copied()
        .enumerate()
        .map(|(index, radius_um)| {
            let k = area_um2 * work.inverse_visible_fraction_sums[index] / denominator;
            let l = (k / std::f64::consts::PI).sqrt();
            if !k.is_finite() || !l.is_finite() {
                return Err(IsotropicSpatialError::InvalidPattern {
                    reason: "isotropic K/L is non-finite".into(),
                });
            }
            Ok(IsotropicKlPoint {
                radius_um,
                directed_pairs: work.directed_pairs[index],
                visible_arc_evaluations: work.visible_arc_evaluations[index],
                visible_arc_fraction_sum: canonical_zero(work.visible_fraction_sums[index]),
                inverse_visible_arc_fraction_sum: canonical_zero(
                    work.inverse_visible_fraction_sums[index],
                ),
                k: Some(canonical_zero(k)),
                l: Some(canonical_zero(l)),
                theoretical_k: std::f64::consts::PI * radius_um * radius_um,
                theoretical_l: radius_um,
                lower_l: None,
                upper_l: None,
            })
        })
        .collect()
}

fn unavailable_curve(radii_um: &[f64]) -> Vec<IsotropicKlPoint> {
    radii_um
        .iter()
        .copied()
        .map(|radius_um| IsotropicKlPoint {
            radius_um,
            directed_pairs: 0,
            visible_arc_evaluations: 0,
            visible_arc_fraction_sum: 0.0,
            inverse_visible_arc_fraction_sum: 0.0,
            k: None,
            l: None,
            theoretical_k: std::f64::consts::PI * radius_um * radius_um,
            theoretical_l: radius_um,
            lower_l: None,
            upper_l: None,
        })
        .collect()
}

struct IsotropicWorkBudget {
    limits: IsotropicSpatialLimits,
    segment_count: usize,
    pair_visits: usize,
    visible_arc_evaluations: usize,
    arc_segment_tests: usize,
    arc_membership_queries: usize,
    maximum_intersection_angles: usize,
}

impl IsotropicWorkBudget {
    fn new(limits: IsotropicSpatialLimits, window: &ObservationWindow2D) -> Self {
        Self {
            limits,
            segment_count: window.translation_segment_count(),
            pair_visits: 0,
            visible_arc_evaluations: 0,
            arc_segment_tests: 0,
            arc_membership_queries: 0,
            maximum_intersection_angles: 0,
        }
    }

    fn charge_pair(&mut self) -> Result<(), IsotropicSpatialError> {
        self.pair_visits = self
            .pair_visits
            .checked_add(1)
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
        if self.pair_visits > self.limits.maximum_pair_visits {
            return Err(IsotropicSpatialError::PairVisitLimitExceeded {
                observed: self.pair_visits,
                maximum: self.limits.maximum_pair_visits,
            });
        }
        Ok(())
    }

    fn visible_fraction(
        &mut self,
        window: &ObservationWindow2D,
        center_x: f64,
        center_y: f64,
        radius: f64,
        center: usize,
        neighbor: usize,
    ) -> Result<f64, IsotropicSpatialError> {
        self.visible_arc_evaluations = self
            .visible_arc_evaluations
            .checked_add(1)
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
        if self.visible_arc_evaluations > self.limits.maximum_visible_arc_evaluations {
            return Err(IsotropicSpatialError::VisibleArcEvaluationLimitExceeded {
                observed: self.visible_arc_evaluations,
                maximum: self.limits.maximum_visible_arc_evaluations,
            });
        }
        self.arc_segment_tests = self
            .arc_segment_tests
            .checked_add(self.segment_count)
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
        if self.arc_segment_tests > self.limits.maximum_arc_segment_tests {
            return Err(IsotropicSpatialError::ArcSegmentTestLimitExceeded {
                required: self.arc_segment_tests,
                maximum: self.limits.maximum_arc_segment_tests,
            });
        }
        let remaining_queries = self
            .limits
            .maximum_arc_membership_queries
            .saturating_sub(self.arc_membership_queries);
        let arc = window
            .visible_circle_arc_fraction(center_x, center_y, radius, remaining_queries)
            .map_err(|error| match error {
                ObservationWindowError::VisibleArcMembershipQueryLimitExceeded { .. } => {
                    IsotropicSpatialError::ArcMembershipQueryLimitExceeded {
                        maximum: self.limits.maximum_arc_membership_queries,
                    }
                }
                other => IsotropicSpatialError::Window(other),
            })?;
        self.arc_membership_queries = self
            .arc_membership_queries
            .checked_add(arc.membership_queries)
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
        self.maximum_intersection_angles = self
            .maximum_intersection_angles
            .max(arc.boundary_intersection_angles);
        if arc.fraction <= 0.0 {
            return Err(IsotropicSpatialError::NonPositiveVisibleArc { center, neighbor });
        }
        Ok(arc.fraction)
    }
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
    config: &IsotropicSpatialConfig,
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
    let angular_bytes = window
        .translation_segment_count()
        .checked_mul(2)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(IsotropicSpatialError::SizeOverflow)?;
    let curve_bytes = config
        .radii_um
        .len()
        .checked_mul(std::mem::size_of::<IsotropicKlPoint>())
        .and_then(|value| value.checked_mul(5))
        .ok_or(IsotropicSpatialError::SizeOverflow)?;
    let matrix_bytes = config
        .simulations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(IsotropicSpatialError::SizeOverflow)?;
    point_bytes
        .checked_add(window_bytes)
        .and_then(|value| value.checked_add(angular_bytes))
        .and_then(|value| value.checked_add(curve_bytes))
        .and_then(|value| value.checked_add(matrix_bytes))
        .ok_or(IsotropicSpatialError::SizeOverflow)
}

pub(super) fn isotropic_configuration_digest(config: &IsotropicSpatialConfig) -> ContentDigest {
    let mut fields = vec![
        b"marklab-isotropic-spatial-configuration-v1".to_vec(),
        (config.radii_um.len() as u128).to_be_bytes().to_vec(),
    ];
    fields.extend(
        config
            .radii_um
            .iter()
            .map(|radius| radius.to_bits().to_be_bytes().to_vec()),
    );
    fields.extend([
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

fn cumulative_counts(values: &mut [usize]) -> Result<(), IsotropicSpatialError> {
    for index in 1..values.len() {
        values[index] = values[index]
            .checked_add(values[index - 1])
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
    }
    Ok(())
}

fn cumulative_finite(values: &mut [f64]) -> Result<(), IsotropicSpatialError> {
    for index in 1..values.len() {
        values[index] += values[index - 1];
        if !values[index].is_finite() {
            return Err(IsotropicSpatialError::InvalidPattern {
                reason: "isotropic cumulative sum is non-finite".into(),
            });
        }
    }
    Ok(())
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

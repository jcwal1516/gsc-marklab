use marklab_workflow::ContentDigest;

use crate::{
    classical::{
        sample_conditional_csr, window_summary, ClassicalInferenceSummary, ClassicalNullDesign,
        ClassicalNullModel, ClassicalRandomizationUnit, ClassicalSpatialLimits,
        SpatialGeometryPlan2D,
    },
    common::seeds::{derive_seed, SeedEndpoint},
    data::Pattern,
    geom::window::ObservationWindow2D,
    permutation::envelopes::GlobalEnvelope,
};

use super::types::*;

/// Compute exact homogeneous translation-corrected K/L with conditional-CSR inference.
pub fn analyze_translation_spatial_pattern(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &TranslationSpatialConfig,
) -> Result<TranslationSpatialResult, TranslationSpatialError> {
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
    .map_err(|error| TranslationSpatialError::InvalidPattern {
        reason: error.to_string(),
    })?;
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, classical_limits)
            .map_err(|error| TranslationSpatialError::InvalidPattern {
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
    let configuration = TranslationConfigurationSummary {
        radius_count: config.radii_um.len(),
        logical_digest: translation_configuration_digest(config).to_string(),
        limits: config.limits,
    };
    let mut geometry = TranslationGeometrySummary {
        point_count: pattern.len(),
        maximum_radius_um,
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
        pair_traversal: "exact_streaming_unordered_pairs_with_both_ordered_weights".into(),
    };
    if pattern.len() < 2 {
        return Ok(TranslationSpatialResult {
            case_id: pattern.meta.case_id.clone(),
            timepoint: pattern.meta.timepoint.clone(),
            correction: "translation".into(),
            status: TranslationSpatialStatus::InsufficientPoints,
            window: window_summary(window.descriptor()),
            geometry,
            configuration,
            null_design,
            curve: unavailable_curve(&config.radii_um),
            inference: None,
            csr_candidate_draws: 0,
        });
    }

    let mut budget = TranslationWorkBudget::new(config.limits, window)?;
    let observed = translation_curve(
        &pattern.x_um,
        &pattern.y_um,
        window,
        &config.radii_um,
        &mut budget,
    )?;
    geometry.observed_pair_visits = budget.pair_visits;
    geometry.observed_overlap_evaluations = budget.overlap_evaluations;
    let mut curve = make_curve(
        window.area_um2(),
        pattern.len(),
        &config.radii_um,
        &observed,
    )?;
    let mut simulated_l = Vec::new();
    simulated_l
        .try_reserve_exact(config.simulations)
        .map_err(|_| TranslationSpatialError::AllocationFailed)?;
    let mut csr_draws = 0_usize;
    for simulation in 0..config.simulations {
        let seed = derive_seed(config.seed, SeedEndpoint::TranslationSpatialCsr, simulation);
        let (x, y) = sample_conditional_csr(
            window,
            pattern.len(),
            seed,
            &mut csr_draws,
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
        let counts = translation_curve(&x, &y, window, &config.radii_um, &mut budget)?;
        let simulated = make_curve(window.area_um2(), pattern.len(), &config.radii_um, &counts)?;
        simulated_l.push(
            simulated
                .into_iter()
                .map(|point| point.l.expect("two-point translation curve is available"))
                .collect(),
        );
    }
    let observed_l = curve
        .iter()
        .map(|point| point.l.expect("two-point translation curve is available"))
        .collect::<Vec<_>>();
    let eligibility = vec![true; observed_l.len()];
    let envelope = GlobalEnvelope::from_curves_with_eligibility(
        &observed_l,
        &simulated_l,
        config.alpha,
        &eligibility,
    )
    .map_err(|error| TranslationSpatialError::Inference {
        reason: error.to_string(),
    })?;
    for (index, point) in curve.iter_mut().enumerate() {
        point.lower_l = Some(canonical_zero(envelope.lower[index]));
        point.upper_l = Some(canonical_zero(envelope.upper[index]));
    }
    geometry.total_pair_visits = budget.pair_visits;
    geometry.total_overlap_evaluations = budget.overlap_evaluations;
    geometry.total_overlap_candidate_work = budget.overlap_candidate_work;
    geometry.maximum_overlap_output_vertices_observed = budget.maximum_output_vertices;
    Ok(TranslationSpatialResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        correction: "translation".into(),
        status: TranslationSpatialStatus::Available,
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

struct TranslationCurveWork {
    unordered_pairs: Vec<usize>,
    overlap_evaluations: Vec<usize>,
    overlap_area_sums: Vec<f64>,
    ordered_weight_sums: Vec<f64>,
}

fn translation_curve(
    x: &[f64],
    y: &[f64],
    window: &ObservationWindow2D,
    radii_um: &[f64],
    budget: &mut TranslationWorkBudget,
) -> Result<TranslationCurveWork, TranslationSpatialError> {
    let mut pair_starts = vec![0_usize; radii_um.len()];
    let mut overlap_starts = vec![0_usize; radii_um.len()];
    let mut overlap_area_starts = vec![0.0_f64; radii_um.len()];
    let mut ordered_weight_starts = vec![0.0_f64; radii_um.len()];
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
            let overlap = budget.overlap(window, displacement_x, displacement_y, left, right)?;
            pair_starts[start] = pair_starts[start]
                .checked_add(1)
                .ok_or(TranslationSpatialError::SizeOverflow)?;
            overlap_starts[start] = overlap_starts[start]
                .checked_add(1)
                .ok_or(TranslationSpatialError::SizeOverflow)?;
            overlap_area_starts[start] += overlap;
            ordered_weight_starts[start] += 2.0 * window.area_um2() / overlap;
            if !overlap_area_starts[start].is_finite() || !ordered_weight_starts[start].is_finite()
            {
                return Err(TranslationSpatialError::InvalidPattern {
                    reason: "translation pair accumulation is non-finite".into(),
                });
            }
        }
    }
    cumulative_counts(&mut pair_starts)?;
    cumulative_counts(&mut overlap_starts)?;
    cumulative_finite(&mut overlap_area_starts)?;
    cumulative_finite(&mut ordered_weight_starts)?;
    Ok(TranslationCurveWork {
        unordered_pairs: pair_starts,
        overlap_evaluations: overlap_starts,
        overlap_area_sums: overlap_area_starts,
        ordered_weight_sums: ordered_weight_starts,
    })
}

fn make_curve(
    area_um2: f64,
    point_count: usize,
    radii_um: &[f64],
    work: &TranslationCurveWork,
) -> Result<Vec<TranslationKlPoint>, TranslationSpatialError> {
    let denominator = point_count
        .checked_mul(point_count - 1)
        .ok_or(TranslationSpatialError::SizeOverflow)? as f64;
    radii_um
        .iter()
        .copied()
        .enumerate()
        .map(|(index, radius_um)| {
            let k = area_um2 * work.ordered_weight_sums[index] / denominator;
            let l = (k / std::f64::consts::PI).sqrt();
            if !k.is_finite() || !l.is_finite() {
                return Err(TranslationSpatialError::InvalidPattern {
                    reason: "translation-corrected K/L is non-finite".into(),
                });
            }
            Ok(TranslationKlPoint {
                radius_um,
                ordered_pairs: work.unordered_pairs[index]
                    .checked_mul(2)
                    .ok_or(TranslationSpatialError::SizeOverflow)?,
                overlap_evaluations: work.overlap_evaluations[index],
                translation_overlap_area_sum_um2: canonical_zero(work.overlap_area_sums[index]),
                translation_weight_sum: canonical_zero(work.ordered_weight_sums[index]),
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

fn unavailable_curve(radii_um: &[f64]) -> Vec<TranslationKlPoint> {
    radii_um
        .iter()
        .copied()
        .map(|radius_um| TranslationKlPoint {
            radius_um,
            ordered_pairs: 0,
            overlap_evaluations: 0,
            translation_overlap_area_sum_um2: 0.0,
            translation_weight_sum: 0.0,
            k: None,
            l: None,
            theoretical_k: std::f64::consts::PI * radius_um * radius_um,
            theoretical_l: radius_um,
            lower_l: None,
            upper_l: None,
        })
        .collect()
}

struct TranslationWorkBudget {
    limits: TranslationSpatialLimits,
    candidate_work_per_overlap: usize,
    pair_visits: usize,
    overlap_evaluations: usize,
    overlap_candidate_work: usize,
    maximum_output_vertices: usize,
}

impl TranslationWorkBudget {
    fn new(
        limits: TranslationSpatialLimits,
        window: &ObservationWindow2D,
    ) -> Result<Self, TranslationSpatialError> {
        let segments = window.translation_segment_count();
        let candidate_work_per_overlap = segments
            .checked_mul(segments)
            .ok_or(TranslationSpatialError::SizeOverflow)?;
        Ok(Self {
            limits,
            candidate_work_per_overlap,
            pair_visits: 0,
            overlap_evaluations: 0,
            overlap_candidate_work: 0,
            maximum_output_vertices: 0,
        })
    }

    fn charge_pair(&mut self) -> Result<(), TranslationSpatialError> {
        self.pair_visits = self
            .pair_visits
            .checked_add(1)
            .ok_or(TranslationSpatialError::SizeOverflow)?;
        if self.pair_visits > self.limits.maximum_pair_visits {
            return Err(TranslationSpatialError::PairVisitLimitExceeded {
                observed: self.pair_visits,
                maximum: self.limits.maximum_pair_visits,
            });
        }
        Ok(())
    }

    fn overlap(
        &mut self,
        window: &ObservationWindow2D,
        displacement_x: f64,
        displacement_y: f64,
        left: usize,
        right: usize,
    ) -> Result<f64, TranslationSpatialError> {
        self.overlap_evaluations = self
            .overlap_evaluations
            .checked_add(1)
            .ok_or(TranslationSpatialError::SizeOverflow)?;
        if self.overlap_evaluations > self.limits.maximum_overlap_evaluations {
            return Err(TranslationSpatialError::OverlapEvaluationLimitExceeded {
                observed: self.overlap_evaluations,
                maximum: self.limits.maximum_overlap_evaluations,
            });
        }
        self.overlap_candidate_work = self
            .overlap_candidate_work
            .checked_add(self.candidate_work_per_overlap)
            .ok_or(TranslationSpatialError::SizeOverflow)?;
        if self.overlap_candidate_work > self.limits.maximum_overlap_candidate_work {
            return Err(TranslationSpatialError::OverlapCandidateWorkLimitExceeded {
                required: self.overlap_candidate_work,
                maximum: self.limits.maximum_overlap_candidate_work,
            });
        }
        let overlap = window.translation_overlap_area_um2(
            displacement_x,
            displacement_y,
            self.limits.maximum_overlap_output_vertices,
        )?;
        self.maximum_output_vertices = self.maximum_output_vertices.max(overlap.output_vertices);
        if overlap.area_um2 <= 0.0 {
            return Err(TranslationSpatialError::NonPositiveOverlap { left, right });
        }
        Ok(overlap.area_um2)
    }
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

fn preflight_overlap_complexity(
    window: &ObservationWindow2D,
    limits: TranslationSpatialLimits,
) -> Result<(), TranslationSpatialError> {
    let segments = window.translation_segment_count();
    let maximum_output = segments
        .checked_mul(segments)
        .and_then(|value| value.checked_add(segments.checked_mul(2)?))
        .ok_or(TranslationSpatialError::SizeOverflow)?;
    if maximum_output > limits.maximum_overlap_output_vertices {
        return Err(TranslationSpatialError::InvalidConfig {
            reason: format!(
                "translation Boolean output bound {maximum_output} exceeds maximum {}",
                limits.maximum_overlap_output_vertices
            ),
        });
    }
    Ok(())
}

fn retained_byte_estimate(
    point_count: usize,
    window: &ObservationWindow2D,
    config: &TranslationSpatialConfig,
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
        .checked_mul(std::mem::size_of::<TranslationKlPoint>())
        .and_then(|value| value.checked_mul(5))
        .ok_or(TranslationSpatialError::SizeOverflow)?;
    let matrix_bytes = config
        .simulations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
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
        .and_then(|value| value.checked_add(overlap_bytes))
        .ok_or(TranslationSpatialError::SizeOverflow)
}

pub(super) fn translation_configuration_digest(config: &TranslationSpatialConfig) -> ContentDigest {
    let mut fields = vec![
        b"marklab-translation-spatial-configuration-v1".to_vec(),
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

fn cumulative_counts(values: &mut [usize]) -> Result<(), TranslationSpatialError> {
    for index in 1..values.len() {
        values[index] = values[index]
            .checked_add(values[index - 1])
            .ok_or(TranslationSpatialError::SizeOverflow)?;
    }
    Ok(())
}

fn cumulative_finite(values: &mut [f64]) -> Result<(), TranslationSpatialError> {
    for index in 1..values.len() {
        values[index] += values[index - 1];
        if !values[index].is_finite() {
            return Err(TranslationSpatialError::InvalidPattern {
                reason: "translation cumulative sum is non-finite".into(),
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

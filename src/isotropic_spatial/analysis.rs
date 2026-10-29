use crate::edge_corrected::{
    estimate_curve,
    inference::{conditional_csr_envelope, CsrError, CsrResult, CurveEvaluation},
    CurveWork,
};
use crate::result_vocabulary::{
    ArcMeasureOwner, DuplicatePointPolicy, EdgeCorrection, PairTraversal, SpatialExecutionMode,
};
use marklab_project::ContentDigest;

use crate::common::finite::canonical_zero;

use crate::{
    classical::{
        window_summary, ClassicalNullDesign, ClassicalNullModel, ClassicalRandomizationUnit,
        ClassicalSpatialLimits, SpatialGeometryPlan2D,
    },
    common::seeds::SeedEndpoint,
    data::Pattern,
    geom::window::ObservationWindow2D,
};

use super::{edge::IsotropicArcBudget, types::*};

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
        execution_mode: SpatialExecutionMode::Exact,
        duplicate_policy: DuplicatePointPolicy::Reject,
        visible_arc_owner: ArcMeasureOwner::ObservationWindow2dSegmentCirclePartition,
        pair_traversal: PairTraversal::ExactStreamingUnorderedPairsWithTwoDirectedArcWeights,
    };
    if pattern.len() < 2 {
        return Ok(IsotropicSpatialResult {
            case_id: pattern.meta.case_id.clone(),
            timepoint: pattern.meta.timepoint.clone(),
            correction: EdgeCorrection::Isotropic,
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

    let mut budget = IsotropicArcBudget::new(config.limits, window);
    let evaluate = |x: &[f64], y: &[f64], budget: &mut IsotropicArcBudget| {
        let work = isotropic_curve(x, y, window, &config.radii_um, budget)?;
        let points = make_curve(window.area_um2(), pattern.len(), &config.radii_um, &work)?;
        let values = points
            .iter()
            .map(|point| point.l.expect("two-point isotropic curve is available"))
            .collect();
        Ok(CurveEvaluation {
            points,
            values,
            eligible: vec![true; config.radii_um.len()],
        })
    };
    let observed = evaluate(&pattern.x_um, &pattern.y_um, &mut budget)?;
    geometry.observed_pair_visits = budget.pair_visits;
    geometry.observed_visible_arc_evaluations = budget.visible_arc_evaluations;
    let CsrResult {
        curve,
        inference,
        candidate_draws: csr_draws,
    } = conditional_csr_envelope(
        window,
        &null_design,
        classical_limits,
        SeedEndpoint::IsotropicSpatialCsr,
        observed,
        |x, y| evaluate(x, y, &mut budget),
        |point, lower, upper| {
            point.lower_l = Some(lower);
            point.upper_l = Some(upper);
        },
    )
    .map_err(|error| match error {
        CsrError::Allocation => IsotropicSpatialError::AllocationFailed,
        CsrError::Simulation(reason) => IsotropicSpatialError::Csr { reason },
        CsrError::Evaluation(error) => error,
        CsrError::Inference(reason) => IsotropicSpatialError::Inference { reason },
    })?;
    geometry.total_pair_visits = budget.pair_visits;
    geometry.total_visible_arc_evaluations = budget.visible_arc_evaluations;
    geometry.total_arc_segment_tests = budget.arc_segment_tests;
    geometry.total_arc_membership_queries = budget.arc_membership_queries;
    geometry.maximum_boundary_intersection_angles = budget.maximum_intersection_angles;
    Ok(IsotropicSpatialResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        correction: EdgeCorrection::Isotropic,
        status: IsotropicSpatialStatus::Available,
        window: window_summary(window.descriptor()),
        geometry,
        configuration,
        null_design,
        curve,
        inference,
        csr_candidate_draws: csr_draws,
    })
}

fn isotropic_curve(
    x: &[f64],
    y: &[f64],
    window: &ObservationWindow2D,
    radii_um: &[f64],
    budget: &mut IsotropicArcBudget,
) -> Result<CurveWork, IsotropicSpatialError> {
    estimate_curve(x, y, window, radii_um, None, budget)
}

fn make_curve(
    area_um2: f64,
    point_count: usize,
    radii_um: &[f64],
    work: &CurveWork,
) -> Result<Vec<IsotropicKlPoint>, IsotropicSpatialError> {
    let denominator = point_count
        .checked_mul(point_count - 1)
        .ok_or(IsotropicSpatialError::SizeOverflow)? as f64;
    radii_um
        .iter()
        .copied()
        .enumerate()
        .map(|(index, radius_um)| {
            let k = area_um2 * work.weighted_sums[index] / denominator;
            let l = (k / std::f64::consts::PI).sqrt();
            if !k.is_finite() || !l.is_finite() {
                return Err(IsotropicSpatialError::InvalidPattern {
                    reason: "isotropic K/L is non-finite".into(),
                });
            }
            Ok(IsotropicKlPoint {
                radius_um,
                directed_pairs: work.pair_counts[index],
                visible_arc_evaluations: work.evaluations[index],
                visible_arc_fraction_sum: canonical_zero(work.measure_sums[index]),
                inverse_visible_arc_fraction_sum: canonical_zero(work.weighted_sums[index]),
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

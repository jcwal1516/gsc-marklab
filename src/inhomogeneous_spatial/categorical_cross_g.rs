use marklab_data::MeasurementStatus;
use marklab_workflow::ContentDigest;

use crate::{
    classical::{window_summary, SpatialGeometryPlan2D},
    common::seeds::{derive_seed, SeedEndpoint},
    pair_correlation::epanechnikov_weight,
    permutation::envelopes::GlobalEnvelope,
    DeclaredScalarPatternInput, ObservationWindow2D, PairCorrelationKernel,
    PairCorrelationPointStatus, Pattern, ScalarMarkId,
};

use super::{
    analysis::{canonical_zero, compensated_add, Counters},
    categorical_cross_g_types::*,
    identity::{
        configuration_digest as intensity_configuration_digest, into_intensity_summary,
        retained_bytes,
    },
    intensity::{evaluate_fixed_intensities, fit_intensity, sample_fixed_grid_pattern},
    pair::geometry_limits,
};

struct Evaluation {
    points: Vec<InhomogeneousCategoricalCrossPairCorrelationPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
    pair_visits: usize,
}

pub fn inhomogeneous_categorical_cross_pair_correlation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &InhomogeneousCategoricalCrossPairCorrelationConfig,
) -> Result<
    InhomogeneousCategoricalCrossPairCorrelationResult,
    InhomogeneousCategoricalCrossPairCorrelationError,
> {
    let frame = window
        .coordinate_frame_id()
        .ok_or(InhomogeneousCategoricalCrossPairCorrelationError::CoordinateFrameMismatch)?;
    if frame != input.coordinate_frame_id() {
        return Err(InhomogeneousCategoricalCrossPairCorrelationError::CoordinateFrameMismatch);
    }
    let mark_id = ScalarMarkId::new("histologic_compartment").map_err(dependency)?;
    let table = input
        .mark_table()
        .ok_or(InhomogeneousCategoricalCrossPairCorrelationError::MissingCategoricalMark)?;
    let codes = table
        .categorical_values(&mark_id)
        .ok_or(InhomogeneousCategoricalCrossPairCorrelationError::MissingCategoricalMark)?;
    let levels = table
        .categorical_levels(&mark_id)
        .ok_or(InhomogeneousCategoricalCrossPairCorrelationError::MissingCategoricalMark)?;
    let source_code = resolve(levels, config.source_level())?;
    let target_code = resolve(levels, config.target_level())?;
    let source_rows = matching_rows(codes, source_code)?;
    let target_rows = matching_rows(codes, target_code)?;
    require_two(config.source_level(), source_rows.len())?;
    require_two(config.target_level(), target_rows.len())?;
    let base = config.intensity_config();
    let pattern = input.pattern();
    if pattern.len() > base.limits().maximum_points
        || pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|value| *value != 1)
    {
        return Err(
            InhomogeneousCategoricalCrossPairCorrelationError::Dependency(
                "pattern shape, validity, or point limit is invalid".into(),
            ),
        );
    }
    let source_pattern = subset_pattern(pattern, &source_rows)?;
    let target_pattern = subset_pattern(pattern, &target_rows)?;
    let all_bytes = retained_bytes(pattern.len(), base)?;
    let source_bytes = retained_bytes(source_pattern.len(), base)?;
    let target_bytes = retained_bytes(target_pattern.len(), base)?;
    let required_bytes = all_bytes
        .checked_add(source_bytes)
        .and_then(|value| value.checked_add(target_bytes))
        .ok_or(InhomogeneousCategoricalCrossPairCorrelationError::SizeOverflow)?;
    if required_bytes > base.limits().maximum_retained_bytes {
        return Err(
            InhomogeneousCategoricalCrossPairCorrelationError::RetainedByteLimitExceeded {
                required: required_bytes,
                maximum: base.limits().maximum_retained_bytes,
            },
        );
    }
    let mut counters = Counters {
        intensity_evaluations: 0,
        pair_visits: 0,
        null_draws: 0,
    };
    let source_fitted = fit_intensity(&source_pattern, window, base, &mut counters)?;
    let target_fitted = fit_intensity(&target_pattern, window, base, &mut counters)?;
    let geometry_limits = geometry_limits(base)?;
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, geometry_limits)
            .map_err(dependency)?;
    let mut source_intensities = filled_vec(pattern.len(), None)?;
    let mut target_intensities = filled_vec(pattern.len(), None)?;
    for (&row, &intensity) in source_rows.iter().zip(&source_fitted.observed_intensities) {
        source_intensities[row] = Some(intensity);
    }
    for (&row, &intensity) in target_rows.iter().zip(&target_fitted.observed_intensities) {
        target_intensities[row] = Some(intensity);
    }
    let mut observed = evaluate(
        &observed_plan,
        &source_rows,
        &target_intensities,
        &source_intensities,
        base.radii_um(),
        config.pair_bandwidth_um(),
        base.limits().maximum_pair_visits,
        &mut counters,
    )?;
    let observed_pair_visits = observed.pair_visits;
    let mut simulations = Vec::new();
    simulations
        .try_reserve_exact(base.simulations())
        .map_err(|_| InhomogeneousCategoricalCrossPairCorrelationError::AllocationFailed)?;
    let mut jointly_eligible = observed.eligible.clone();
    for simulation in 0..base.simulations() {
        let (source_x, source_y) = sample_fixed_grid_pattern(
            window,
            source_rows.len(),
            &source_fitted.grid,
            &source_fitted.probe_cdf,
            source_fitted.fixed_grid_total_mass,
            derive_seed(
                base.seed(),
                SeedEndpoint::InhomogeneousCrossSourceNull,
                simulation,
            ),
            base,
            &mut counters,
        )?;
        let (target_x, target_y) = sample_fixed_grid_pattern(
            window,
            target_rows.len(),
            &target_fitted.grid,
            &target_fitted.probe_cdf,
            target_fitted.fixed_grid_total_mass,
            derive_seed(
                base.seed(),
                SeedEndpoint::InhomogeneousCrossTargetNull,
                simulation,
            ),
            base,
            &mut counters,
        )?;
        let simulated_source_intensity = evaluate_fixed_intensities(
            &source_x,
            &source_y,
            &source_pattern,
            &source_fitted,
            base,
            &mut counters,
        )?;
        let simulated_target_intensity = evaluate_fixed_intensities(
            &target_x,
            &target_y,
            &target_pattern,
            &target_fitted,
            base,
            &mut counters,
        )?;
        let source_count = source_x.len();
        let mut x = source_x;
        let mut y = source_y;
        x.try_reserve_exact(target_x.len())
            .and_then(|()| y.try_reserve_exact(target_y.len()))
            .map_err(|_| InhomogeneousCategoricalCrossPairCorrelationError::AllocationFailed)?;
        x.extend(target_x);
        y.extend(target_y);
        let plan =
            SpatialGeometryPlan2D::new(&x, &y, window, geometry_limits).map_err(dependency)?;
        let mut simulated_source_rows = Vec::new();
        simulated_source_rows
            .try_reserve_exact(source_count)
            .map_err(|_| InhomogeneousCategoricalCrossPairCorrelationError::AllocationFailed)?;
        simulated_source_rows.extend(0..source_count);
        let mut source_map = filled_vec(x.len(), None)?;
        let mut target_map = filled_vec(x.len(), None)?;
        for (row, intensity) in simulated_source_intensity.into_iter().enumerate() {
            source_map[row] = Some(intensity);
        }
        for (offset, intensity) in simulated_target_intensity.into_iter().enumerate() {
            target_map[source_count + offset] = Some(intensity);
        }
        let evaluated = evaluate(
            &plan,
            &simulated_source_rows,
            &target_map,
            &source_map,
            base.radii_um(),
            config.pair_bandwidth_um(),
            base.limits().maximum_pair_visits,
            &mut counters,
        )?;
        for (joint, current) in jointly_eligible.iter_mut().zip(evaluated.eligible) {
            *joint &= current;
        }
        simulations.push(evaluated.values);
    }
    let (p_global, erl_depth, critical_depth, eligible_radius_count) =
        if jointly_eligible.iter().any(|value| *value) {
            let envelope = GlobalEnvelope::from_curves_with_eligibility(
                &observed.values,
                &simulations,
                base.alpha(),
                &jointly_eligible,
            )
            .map_err(dependency)?;
            for (index, point) in observed.points.iter_mut().enumerate() {
                if jointly_eligible[index] {
                    point.inference_eligible = true;
                    point.lower_cross_g = Some(canonical_zero(envelope.lower[index]));
                    point.upper_cross_g = Some(canonical_zero(envelope.upper[index]));
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
    let source_intensity = into_intensity_summary(&source_pattern, window, base, source_fitted)?;
    let target_intensity = into_intensity_summary(&target_pattern, window, base, target_fitted)?;
    Ok(InhomogeneousCategoricalCrossPairCorrelationResult {
        mark_id: mark_id.as_str().into(),
        measurement_status: measurement_status_name(
            table
                .measurement_status(&mark_id)
                .ok_or(InhomogeneousCategoricalCrossPairCorrelationError::MissingCategoricalMark)?,
        )
        .into(),
        coordinate_frame_id: frame.as_str().into(),
        window: window_summary(window.descriptor()),
        source_level: config.source_level().into(),
        target_level: config.target_level().into(),
        source_count: source_rows.len(),
        target_count: target_rows.len(),
        source_rows,
        target_rows,
        source_intensity,
        target_intensity,
        kernel: PairCorrelationKernel::Epanechnikov,
        pair_bandwidth_um: config.pair_bandwidth_um(),
        intensity_bandwidth_um: base.bandwidth_um(),
        edge_correction:
            "standard_border_radius_plus_pair_bandwidth_type_specific_inverse_intensity_ratio"
                .into(),
        configuration_digest: configuration_digest(config).to_string(),
        observed_pair_visits,
        total_pair_visits: counters.pair_visits,
        intensity_evaluations: counters.intensity_evaluations,
        estimated_storage_bytes: required_bytes,
        limits: base.limits(),
        curve: observed.points,
        inference: super::InhomogeneousSpatialInference {
            null_model: "independent_fixed_gridded_type_specific_inhomogeneous_binomial".into(),
            randomization_unit:
                "independent_source_and_target_location_patterns_conditioned_on_type_counts".into(),
            p_global,
            erl_depth,
            critical_depth,
            eligible_radius_count,
            simulations_completed: base.simulations(),
            seed: base.seed(),
            alpha: base.alpha(),
            null_draws: counters.null_draws,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate(
    plan: &SpatialGeometryPlan2D,
    source_rows: &[usize],
    target_intensities: &[Option<f64>],
    source_intensities: &[Option<f64>],
    radii: &[f64],
    pair_bandwidth_um: f64,
    maximum_pair_visits: usize,
    counters: &mut Counters,
) -> Result<Evaluation, InhomogeneousCategoricalCrossPairCorrelationError> {
    let mut centers = filled_vec(radii.len(), 0usize)?;
    let mut center_sums = filled_vec(radii.len(), 0.0)?;
    let mut center_corrections = filled_vec(radii.len(), 0.0)?;
    let mut pairs = filled_vec(radii.len(), 0usize)?;
    let mut kernel_sums = filled_vec(radii.len(), 0.0)?;
    let mut kernel_corrections = filled_vec(radii.len(), 0.0)?;
    let maximum_query = radii[radii.len() - 1] + pair_bandwidth_um;
    let starting_visits = counters.pair_visits;
    for &source in source_rows {
        let source_intensity = source_intensities[source].ok_or_else(|| {
            InhomogeneousCategoricalCrossPairCorrelationError::Dependency(
                "source intensity row is absent".into(),
            )
        })?;
        let inverse_source = 1.0 / source_intensity;
        let boundary = plan.boundary_distances()[source];
        let eligible_end = radii.partition_point(|radius| *radius + pair_bandwidth_um <= boundary);
        for index in 0..eligible_end {
            centers[index] = centers[index]
                .checked_add(1)
                .ok_or(InhomogeneousCategoricalCrossPairCorrelationError::SizeOverflow)?;
            compensated_add(
                &mut center_sums[index],
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
                if let Err(error) = counters.charge_pair(maximum_pair_visits) {
                    visitor_error = Some(error.into());
                    return;
                }
                let Some(target_intensity) = target_intensities[neighbor.index] else {
                    return;
                };
                let lower = neighbor.distance_um - pair_bandwidth_um;
                let upper = neighbor.distance_um + pair_bandwidth_um;
                let start = radii.partition_point(|radius| *radius <= lower);
                let end = radii
                    .partition_point(|radius| *radius < upper)
                    .min(eligible_end);
                for index in start..end {
                    let kernel =
                        epanechnikov_weight(radii[index], neighbor.distance_um, pair_bandwidth_um)
                            .expect("partitioned positive kernel support");
                    let weight = kernel * inverse_source / target_intensity;
                    if !weight.is_finite() || weight <= 0.0 {
                        visitor_error = Some(
                            InhomogeneousCategoricalCrossPairCorrelationError::Dependency(
                                "inhomogeneous cross-g pair weight is invalid".into(),
                            ),
                        );
                        return;
                    }
                    pairs[index] = match pairs[index].checked_add(1) {
                        Some(value) => value,
                        None => {
                            visitor_error = Some(
                                InhomogeneousCategoricalCrossPairCorrelationError::SizeOverflow,
                            );
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
        .map_err(|_| InhomogeneousCategoricalCrossPairCorrelationError::AllocationFailed)?;
    for (index, radius_um) in radii.iter().copied().enumerate() {
        let center_sum = center_sums[index] + center_corrections[index];
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
        let cross_g = (status == PairCorrelationPointStatus::Available)
            .then(|| kernel_sum / (2.0 * std::f64::consts::PI * radius_um * center_sum))
            .map(canonical_zero);
        if cross_g.is_some_and(|value| !value.is_finite() || value < 0.0) {
            return Err(
                InhomogeneousCategoricalCrossPairCorrelationError::Dependency(
                    "inhomogeneous cross-g normalization is invalid".into(),
                ),
            );
        }
        values.push(cross_g.unwrap_or(0.0));
        eligible.push(cross_g.is_some());
        points.push(InhomogeneousCategoricalCrossPairCorrelationPoint {
            radius_um,
            status,
            eligible_source_centers: centers[index],
            directed_source_target_pairs_in_support: pairs[index],
            inverse_intensity_kernel_sum: canonical_zero(kernel_sum),
            eligible_source_inverse_intensity_sum: canonical_zero(center_sum),
            cross_g,
            theoretical_cross_g: 1.0,
            inference_eligible: false,
            lower_cross_g: None,
            upper_cross_g: None,
        });
    }
    Ok(Evaluation {
        points,
        values,
        eligible,
        pair_visits: counters.pair_visits - starting_visits,
    })
}

fn matching_rows(
    codes: &[u32],
    requested: u32,
) -> Result<Vec<usize>, InhomogeneousCategoricalCrossPairCorrelationError> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(codes.len())
        .map_err(|_| InhomogeneousCategoricalCrossPairCorrelationError::AllocationFailed)?;
    rows.extend(
        codes
            .iter()
            .enumerate()
            .filter_map(|(row, code)| (*code == requested).then_some(row)),
    );
    Ok(rows)
}

fn subset_pattern(
    pattern: &Pattern,
    rows: &[usize],
) -> Result<Pattern, InhomogeneousCategoricalCrossPairCorrelationError> {
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut mark = Vec::new();
    x.try_reserve_exact(rows.len())
        .and_then(|()| y.try_reserve_exact(rows.len()))
        .and_then(|()| mark.try_reserve_exact(rows.len()))
        .map_err(|_| InhomogeneousCategoricalCrossPairCorrelationError::AllocationFailed)?;
    x.extend(rows.iter().map(|row| pattern.x_um[*row]));
    y.extend(rows.iter().map(|row| pattern.y_um[*row]));
    mark.resize(rows.len(), 0);
    Pattern::from_arrays(x, y, mark, pattern.meta.clone()).map_err(dependency)
}

fn filled_vec<T: Clone>(
    length: usize,
    value: T,
) -> Result<Vec<T>, InhomogeneousCategoricalCrossPairCorrelationError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| InhomogeneousCategoricalCrossPairCorrelationError::AllocationFailed)?;
    result.resize(length, value);
    Ok(result)
}

fn require_two(
    level: &str,
    count: usize,
) -> Result<(), InhomogeneousCategoricalCrossPairCorrelationError> {
    if count < 2 {
        return Err(
            InhomogeneousCategoricalCrossPairCorrelationError::SparseLevel {
                level: level.into(),
                count,
            },
        );
    }
    Ok(())
}

fn resolve(
    levels: &[String],
    requested: &str,
) -> Result<u32, InhomogeneousCategoricalCrossPairCorrelationError> {
    levels
        .iter()
        .position(|level| level == requested)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| {
            InhomogeneousCategoricalCrossPairCorrelationError::MissingLevel(requested.into())
        })
}

pub(crate) fn configuration_digest(
    config: &InhomogeneousCategoricalCrossPairCorrelationConfig,
) -> ContentDigest {
    let intensity = intensity_configuration_digest(config.intensity_config());
    let bandwidth = config.pair_bandwidth_um().to_bits().to_be_bytes();
    ContentDigest::from_framed([
        b"marklab-inhomogeneous-categorical-cross-g-config-v1".as_slice(),
        intensity.as_bytes(),
        bandwidth.as_slice(),
        config.source_level().as_bytes(),
        config.target_level().as_bytes(),
    ])
}

fn measurement_status_name(status: MeasurementStatus) -> &'static str {
    match status {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

fn dependency(error: impl std::fmt::Display) -> InhomogeneousCategoricalCrossPairCorrelationError {
    InhomogeneousCategoricalCrossPairCorrelationError::Dependency(error.to_string())
}

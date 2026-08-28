use marklab_cohort::{InferenceAlternative, InferenceDesign};
use marklab_data::MeasurementStatus;
use marklab_workflow::ContentDigest;

use crate::{
    classical::window_summary,
    mark_pair_plan::{build_mark_pair_plan, erl_workspace_bytes},
    pair_correlation::epanechnikov_weight,
    permutation::envelopes::GlobalEnvelope,
    DeclaredScalarPatternInput, ObservationWindow2D, PairCorrelationKernel,
    PairCorrelationPointStatus, ScalarMarkId,
};

use super::types::*;

struct Evaluation {
    points: Vec<CategoricalCrossPairCorrelationPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
}

pub fn categorical_cross_pair_correlation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &CategoricalCrossPairCorrelationConfig,
) -> Result<CategoricalCrossPairCorrelationResult, CategoricalCrossPairCorrelationError> {
    let frame = window
        .coordinate_frame_id()
        .ok_or(CategoricalCrossPairCorrelationError::CoordinateFrameMismatch)?;
    if frame != input.coordinate_frame_id() {
        return Err(CategoricalCrossPairCorrelationError::CoordinateFrameMismatch);
    }
    let mark_id = ScalarMarkId::new("histologic_compartment").map_err(dependency)?;
    let table = input
        .mark_table()
        .ok_or(CategoricalCrossPairCorrelationError::MissingCategoricalMark)?;
    let codes = table
        .categorical_values(&mark_id)
        .ok_or(CategoricalCrossPairCorrelationError::MissingCategoricalMark)?;
    let levels = table
        .categorical_levels(&mark_id)
        .ok_or(CategoricalCrossPairCorrelationError::MissingCategoricalMark)?;
    let source_code = resolve(levels, &config.source_level)?;
    let target_code = resolve(levels, &config.target_level)?;
    let source_count = codes.iter().filter(|code| **code == source_code).count();
    let target_count = codes.iter().filter(|code| **code == target_code).count();
    if source_count == 0 || target_count == 0 {
        return Err(CategoricalCrossPairCorrelationError::MissingLevel(
            if source_count == 0 {
                config.source_level.clone()
            } else {
                config.target_level.clone()
            },
        ));
    }
    let support_radius = config.radii_um[config.radii_um.len() - 1] + config.bandwidth_um;
    let plan = build_mark_pair_plan(
        input,
        window,
        &[support_radius],
        config.limits.maximum_points,
        config.limits.maximum_radii,
        config.limits.maximum_directed_pairs,
        config.limits.maximum_retained_bytes,
    )
    .map_err(|error| dependency(format!("{error:?}")))?;
    let permutation_pair_evaluations = plan
        .pairs
        .len()
        .checked_mul(config.permutations)
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    if permutation_pair_evaluations > config.limits.maximum_permutation_pair_evaluations {
        return Err(CategoricalCrossPairCorrelationError::ResourceLimitExceeded);
    }
    let retained_bytes = retained_bytes(codes.len(), plan.retained_bytes, config)?;
    if retained_bytes > config.limits.maximum_retained_bytes {
        return Err(
            CategoricalCrossPairCorrelationError::RetainedByteLimitExceeded {
                required: retained_bytes,
                maximum: config.limits.maximum_retained_bytes,
            },
        );
    }
    let mut observed = evaluate(
        codes,
        source_code,
        target_code,
        target_count,
        window.area_um2(),
        &plan,
        config,
    )?;
    let design = InferenceDesign::random_labeling(
        codes.len(),
        config.permutations,
        config.seed,
        InferenceAlternative::TwoSided,
    )
    .map_err(dependency)?;
    let mut simulations = Vec::new();
    simulations
        .try_reserve_exact(config.permutations)
        .map_err(|_| CategoricalCrossPairCorrelationError::AllocationFailed)?;
    let mut eligible = observed.eligible.clone();
    for replicate in 0..config.permutations {
        let indices = design.permuted_indices(replicate).map_err(dependency)?;
        let mut permuted = Vec::new();
        permuted
            .try_reserve_exact(indices.len())
            .map_err(|_| CategoricalCrossPairCorrelationError::AllocationFailed)?;
        permuted.extend(indices.iter().map(|index| codes[*index]));
        let evaluated = evaluate(
            &permuted,
            source_code,
            target_code,
            target_count,
            window.area_um2(),
            &plan,
            config,
        )?;
        for (joint, current) in eligible.iter_mut().zip(evaluated.eligible) {
            *joint &= current;
        }
        simulations.push(evaluated.values);
    }
    let (p_global, erl_depth, critical_depth, eligible_radius_count) =
        if eligible.iter().any(|value| *value) {
            let envelope = GlobalEnvelope::from_curves_with_eligibility(
                &observed.values,
                &simulations,
                config.alpha,
                &eligible,
            )
            .map_err(dependency)?;
            for (index, point) in observed.points.iter_mut().enumerate() {
                if eligible[index] {
                    point.inference_eligible = true;
                    point.lower_cross_g = Some(canonical_zero(envelope.lower[index]));
                    point.upper_cross_g = Some(canonical_zero(envelope.upper[index]));
                }
            }
            (
                Some(canonical_zero(envelope.p_global)),
                Some(canonical_zero(envelope.erl_depth)),
                Some(canonical_zero(envelope.critical_depth)),
                eligible.iter().filter(|value| **value).count(),
            )
        } else {
            (None, None, None, 0)
        };
    Ok(CategoricalCrossPairCorrelationResult {
        mark_id: mark_id.as_str().into(),
        measurement_status: measurement_status_name(
            table
                .measurement_status(&mark_id)
                .ok_or(CategoricalCrossPairCorrelationError::MissingCategoricalMark)?,
        )
        .into(),
        coordinate_frame_id: frame.as_str().into(),
        window: window_summary(window.descriptor()),
        source_level: config.source_level.clone(),
        target_level: config.target_level.clone(),
        source_count,
        target_count,
        kernel: PairCorrelationKernel::Epanechnikov,
        bandwidth_um: config.bandwidth_um,
        edge_correction: "standard_border_radius_plus_bandwidth".into(),
        geometry_digest: plan.geometry.logical_digest().to_string(),
        pair_plan_digest: plan.digest.to_string(),
        directed_pair_count: plan.pairs.len(),
        geometry_build_count: 1,
        estimated_storage_bytes: retained_bytes,
        configuration_digest: configuration_digest(config).to_string(),
        curve: observed.points,
        inference: CategoricalCrossPairCorrelationInference {
            null_model: "random_labeling".into(),
            permutation_unit: "complete_categorical_row".into(),
            p_global,
            erl_depth,
            critical_depth,
            eligible_radius_count,
            permutations_completed: config.permutations,
            seed: config.seed,
            alpha: config.alpha,
            permutation_pair_evaluations,
        },
    })
}

fn evaluate(
    codes: &[u32],
    source_code: u32,
    target_code: u32,
    target_count: usize,
    area_um2: f64,
    plan: &crate::mark_pair_plan::MarkPairPlan,
    config: &CategoricalCrossPairCorrelationConfig,
) -> Result<Evaluation, CategoricalCrossPairCorrelationError> {
    let mut centers = zeroed_vec::<usize>(config.radii_um.len())?;
    let mut pairs = zeroed_vec::<usize>(config.radii_um.len())?;
    let mut sums = zeroed_vec::<f64>(config.radii_um.len())?;
    let mut compensation = zeroed_vec::<f64>(config.radii_um.len())?;
    for (row, code) in codes.iter().copied().enumerate() {
        if code == source_code {
            let end = config.radii_um.partition_point(|radius| {
                *radius + config.bandwidth_um <= plan.geometry.boundary_distances()[row]
            });
            for value in &mut centers[..end] {
                *value = value
                    .checked_add(1)
                    .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
            }
        }
    }
    for pair in &plan.pairs {
        if codes[pair.source] != source_code || codes[pair.target] != target_code {
            continue;
        }
        let lower = pair.distance_um - config.bandwidth_um;
        let upper = pair.distance_um + config.bandwidth_um;
        let start = config.radii_um.partition_point(|radius| *radius <= lower);
        let end = config.radii_um.partition_point(|radius| *radius < upper);
        for index in start..end {
            let radius = config.radii_um[index];
            if radius + config.bandwidth_um <= plan.geometry.boundary_distances()[pair.source] {
                if let Some(weight) =
                    epanechnikov_weight(radius, pair.distance_um, config.bandwidth_um)
                {
                    pairs[index] = pairs[index]
                        .checked_add(1)
                        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
                    let corrected = weight - compensation[index];
                    let next = sums[index] + corrected;
                    compensation[index] = (next - sums[index]) - corrected;
                    sums[index] = next;
                    if !sums[index].is_finite() {
                        return Err(CategoricalCrossPairCorrelationError::Dependency(
                            "cross-g kernel accumulation is non-finite".into(),
                        ));
                    }
                }
            }
        }
    }
    let mut points = Vec::new();
    let mut values = Vec::new();
    let mut eligible = Vec::new();
    points
        .try_reserve_exact(config.radii_um.len())
        .and_then(|()| values.try_reserve_exact(config.radii_um.len()))
        .and_then(|()| eligible.try_reserve_exact(config.radii_um.len()))
        .map_err(|_| CategoricalCrossPairCorrelationError::AllocationFailed)?;
    for index in 0..config.radii_um.len() {
        let status = if centers[index] == 0 {
            PairCorrelationPointStatus::NoEligibleCenters
        } else if pairs[index] == 0 {
            PairCorrelationPointStatus::NoPairsInKernelSupport
        } else {
            PairCorrelationPointStatus::Available
        };
        let cross_g = (status == PairCorrelationPointStatus::Available).then(|| {
            area_um2 * sums[index]
                / (2.0
                    * std::f64::consts::PI
                    * config.radii_um[index]
                    * centers[index] as f64
                    * target_count as f64)
        });
        if cross_g.is_some_and(|value| !value.is_finite()) {
            return Err(CategoricalCrossPairCorrelationError::Dependency(
                "cross-g normalization is non-finite".into(),
            ));
        }
        values.push(cross_g.unwrap_or(0.0));
        eligible.push(cross_g.is_some());
        points.push(CategoricalCrossPairCorrelationPoint {
            radius_um: config.radii_um[index],
            status,
            eligible_source_centers: centers[index],
            directed_source_target_pairs_in_support: pairs[index],
            kernel_weight_sum: canonical_zero(sums[index]),
            cross_g: cross_g.map(canonical_zero),
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
    })
}

fn retained_bytes(
    rows: usize,
    plan_bytes: usize,
    config: &CategoricalCrossPairCorrelationConfig,
) -> Result<usize, CategoricalCrossPairCorrelationError> {
    let matrix_values = config
        .permutations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let matrix_descriptors = config
        .permutations
        .checked_mul(std::mem::size_of::<Vec<f64>>())
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let permutation_work = rows
        .checked_mul(
            3 * std::mem::size_of::<usize>()
                + std::mem::size_of::<u32>()
                + std::mem::size_of::<bool>(),
        )
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let radius_work = config
        .radii_um
        .len()
        .checked_mul(
            2 * std::mem::size_of::<CategoricalCrossPairCorrelationPoint>()
                + 5 * std::mem::size_of::<usize>()
                + 5 * std::mem::size_of::<f64>()
                + 2 * std::mem::size_of::<bool>(),
        )
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let work = permutation_work
        .checked_add(radius_work)
        .and_then(|value| {
            value.checked_add(std::mem::size_of::<Vec<CategoricalCrossPairCorrelationPoint>>())
        })
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let erl = erl_workspace_bytes(config.permutations, config.radii_um.len())
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    plan_bytes
        .checked_add(matrix_values)
        .and_then(|value| value.checked_add(matrix_descriptors))
        .and_then(|value| value.checked_add(work))
        .and_then(|value| value.checked_add(erl))
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)
}

pub(crate) fn configuration_digest(
    config: &CategoricalCrossPairCorrelationConfig,
) -> ContentDigest {
    let mut fields = vec![b"marklab-categorical-cross-g-config-v1".to_vec()];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        config.bandwidth_um.to_bits().to_be_bytes().to_vec(),
        config.source_level.as_bytes().to_vec(),
        config.target_level.as_bytes().to_vec(),
        (config.permutations as u128).to_be_bytes().to_vec(),
        config.seed.to_be_bytes().to_vec(),
        config.alpha.to_bits().to_be_bytes().to_vec(),
        (config.limits.maximum_points as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_radii as u128).to_be_bytes().to_vec(),
        (config.limits.maximum_directed_pairs as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_permutation_pair_evaluations as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_retained_bytes as u128)
            .to_be_bytes()
            .to_vec(),
    ]);
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

fn zeroed_vec<T: Default + Clone>(
    length: usize,
) -> Result<Vec<T>, CategoricalCrossPairCorrelationError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| CategoricalCrossPairCorrelationError::AllocationFailed)?;
    values.resize(length, T::default());
    Ok(values)
}

fn measurement_status_name(status: MeasurementStatus) -> &'static str {
    match status {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

fn resolve(
    levels: &[String],
    requested: &str,
) -> Result<u32, CategoricalCrossPairCorrelationError> {
    levels
        .iter()
        .position(|level| level == requested)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| CategoricalCrossPairCorrelationError::MissingLevel(requested.into()))
}

fn dependency(error: impl std::fmt::Display) -> CategoricalCrossPairCorrelationError {
    CategoricalCrossPairCorrelationError::Dependency(error.to_string())
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

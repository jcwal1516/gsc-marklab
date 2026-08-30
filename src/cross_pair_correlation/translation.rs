use std::collections::BTreeMap;

use marklab_cohort::{InferenceAlternative, InferenceDesign};
use marklab_data::MeasurementStatus;
use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};

use crate::{
    classical::window_summary,
    mark_pair_plan::{build_mark_pair_plan, erl_workspace_bytes, MarkPairPlan},
    pair_correlation::epanechnikov_weight,
    permutation::envelopes::GlobalEnvelope,
    translation_spatial::edge::{preflight_overlap_complexity, TranslationOverlapBudget},
    ClassicalWindowSummary, DeclaredScalarPatternInput, ObservationWindow2D, PairCorrelationKernel,
    PairCorrelationPointStatus, ScalarMarkId, TranslationSpatialError, TranslationSpatialLimits,
};

use super::{CategoricalCrossPairCorrelationError, CategoricalCrossPairCorrelationInference};

/// Standard categorical cross-g controls plus exact polygon-translation ceilings.
#[derive(Clone, Debug, PartialEq)]
pub struct TranslationCategoricalCrossPairCorrelationConfig {
    base: super::CategoricalCrossPairCorrelationConfig,
    translation_limits: TranslationSpatialLimits,
}

impl TranslationCategoricalCrossPairCorrelationConfig {
    /// Bind an already validated directed categorical design to exact translation work limits.
    pub fn new(
        base: super::CategoricalCrossPairCorrelationConfig,
        translation_limits: TranslationSpatialLimits,
    ) -> Result<Self, CategoricalCrossPairCorrelationError> {
        if translation_limits.maximum_points < base.limits.maximum_points
            || translation_limits.maximum_radii < base.radii_um.len()
            || translation_limits.maximum_retained_bytes < base.limits.maximum_retained_bytes
        {
            return Err(CategoricalCrossPairCorrelationError::InvalidConfig(
                "translation ceilings must cover the categorical point, radius, and memory design"
                    .into(),
            ));
        }
        Ok(Self {
            base,
            translation_limits,
        })
    }

    pub fn base(&self) -> &super::CategoricalCrossPairCorrelationConfig {
        &self.base
    }

    pub fn translation_limits(&self) -> TranslationSpatialLimits {
        self.translation_limits
    }
}

/// One radius of exact translation-corrected directed categorical cross-g.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationCategoricalCrossPairCorrelationPoint {
    pub radius_um: f64,
    pub status: PairCorrelationPointStatus,
    pub directed_source_target_pairs_in_support: usize,
    pub overlap_evaluations: usize,
    pub translation_overlap_area_sum_um2: f64,
    pub translation_weighted_kernel_sum: f64,
    pub cross_g: Option<f64>,
    pub theoretical_cross_g: f64,
    pub inference_eligible: bool,
    pub lower_cross_g: Option<f64>,
    pub upper_cross_g: Option<f64>,
}

/// Complete exact translation-corrected directed categorical cross-g result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationCategoricalCrossPairCorrelationResult {
    pub mark_id: String,
    pub measurement_status: String,
    pub coordinate_frame_id: String,
    pub window: ClassicalWindowSummary,
    pub source_level: String,
    pub target_level: String,
    pub source_count: usize,
    pub target_count: usize,
    pub kernel: PairCorrelationKernel,
    pub bandwidth_um: f64,
    pub edge_correction: String,
    pub geometry_digest: String,
    pub pair_plan_digest: String,
    pub retained_directed_pair_count: usize,
    pub observed_unordered_pair_visits: usize,
    pub overlap_evaluations: usize,
    pub overlap_candidate_work: usize,
    pub maximum_overlap_output_vertices_observed: usize,
    pub estimated_storage_bytes: usize,
    pub configuration_digest: String,
    pub curve: Vec<TranslationCategoricalCrossPairCorrelationPoint>,
    pub inference: CategoricalCrossPairCorrelationInference,
}

struct Evaluation {
    points: Vec<TranslationCategoricalCrossPairCorrelationPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
}

/// Estimate exact polygon translation-corrected directed categorical cross-g.
pub fn translation_categorical_cross_pair_correlation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &TranslationCategoricalCrossPairCorrelationConfig,
) -> Result<TranslationCategoricalCrossPairCorrelationResult, CategoricalCrossPairCorrelationError>
{
    let base = &config.base;
    preflight_overlap_complexity(window, config.translation_limits)
        .map_err(translation_dependency)?;
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
    let source_code = resolve(levels, &base.source_level)?;
    let target_code = resolve(levels, &base.target_level)?;
    let source_count = codes.iter().filter(|code| **code == source_code).count();
    let target_count = codes.iter().filter(|code| **code == target_code).count();
    if source_count == 0 || target_count == 0 {
        return Err(CategoricalCrossPairCorrelationError::MissingLevel(
            if source_count == 0 {
                base.source_level.clone()
            } else {
                base.target_level.clone()
            },
        ));
    }
    let support_radius = base.radii_um[base.radii_um.len() - 1] + base.bandwidth_um;
    let plan = build_mark_pair_plan(
        input,
        window,
        &[support_radius],
        base.limits.maximum_points,
        base.limits.maximum_radii,
        base.limits.maximum_directed_pairs,
        base.limits.maximum_retained_bytes,
    )
    .map_err(|error| dependency(format!("{error:?}")))?;
    let permutation_pair_evaluations = plan
        .pairs
        .len()
        .checked_mul(base.permutations)
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    if permutation_pair_evaluations > base.limits.maximum_permutation_pair_evaluations {
        return Err(CategoricalCrossPairCorrelationError::ResourceLimitExceeded);
    }
    let mut budget = TranslationOverlapBudget::new(config.translation_limits, window)
        .map_err(translation_dependency)?;
    let overlaps = build_overlaps(input, window, base, &plan, &mut budget)?;
    let retained_bytes = retained_bytes(codes.len(), plan.retained_bytes, overlaps.len(), config)?;
    if retained_bytes > config.translation_limits.maximum_retained_bytes {
        return Err(
            CategoricalCrossPairCorrelationError::RetainedByteLimitExceeded {
                required: retained_bytes,
                maximum: config.translation_limits.maximum_retained_bytes,
            },
        );
    }
    let mut observed = evaluate(
        codes,
        source_code,
        target_code,
        source_count,
        target_count,
        window.area_um2(),
        &plan,
        &overlaps,
        base,
    )?;
    let design = InferenceDesign::random_labeling(
        codes.len(),
        base.permutations,
        base.seed,
        InferenceAlternative::TwoSided,
    )
    .map_err(dependency)?;
    let mut simulations = Vec::new();
    simulations
        .try_reserve_exact(base.permutations)
        .map_err(|_| CategoricalCrossPairCorrelationError::AllocationFailed)?;
    let mut eligible = observed.eligible.clone();
    for replicate in 0..base.permutations {
        let indices = design.permuted_indices(replicate).map_err(dependency)?;
        let permuted = indices
            .iter()
            .map(|index| codes[*index])
            .collect::<Vec<_>>();
        let evaluated = evaluate(
            &permuted,
            source_code,
            target_code,
            source_count,
            target_count,
            window.area_um2(),
            &plan,
            &overlaps,
            base,
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
                base.alpha,
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
    Ok(TranslationCategoricalCrossPairCorrelationResult {
        mark_id: mark_id.as_str().into(),
        measurement_status: measurement_status_name(
            table
                .measurement_status(&mark_id)
                .ok_or(CategoricalCrossPairCorrelationError::MissingCategoricalMark)?,
        )
        .into(),
        coordinate_frame_id: frame.as_str().into(),
        window: window_summary(window.descriptor()),
        source_level: base.source_level.clone(),
        target_level: base.target_level.clone(),
        source_count,
        target_count,
        kernel: PairCorrelationKernel::Epanechnikov,
        bandwidth_um: base.bandwidth_um,
        edge_correction: "translation".into(),
        geometry_digest: plan.geometry.logical_digest().to_string(),
        pair_plan_digest: plan.digest.to_string(),
        retained_directed_pair_count: plan.pairs.len(),
        observed_unordered_pair_visits: budget.pair_visits,
        overlap_evaluations: budget.overlap_evaluations,
        overlap_candidate_work: budget.overlap_candidate_work,
        maximum_overlap_output_vertices_observed: budget.maximum_output_vertices,
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
            permutations_completed: base.permutations,
            seed: base.seed,
            alpha: base.alpha,
            permutation_pair_evaluations,
        },
    })
}

fn build_overlaps(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &super::CategoricalCrossPairCorrelationConfig,
    plan: &MarkPairPlan,
    budget: &mut TranslationOverlapBudget,
) -> Result<BTreeMap<(usize, usize), f64>, CategoricalCrossPairCorrelationError> {
    let pattern = input.pattern();
    let mut overlaps = BTreeMap::new();
    for pair in &plan.pairs {
        if pair.source >= pair.target {
            continue;
        }
        budget.charge_pair().map_err(translation_dependency)?;
        let lower = pair.distance_um - config.bandwidth_um;
        let upper = pair.distance_um + config.bandwidth_um;
        let start = config.radii_um.partition_point(|radius| *radius <= lower);
        let end = config.radii_um.partition_point(|radius| *radius < upper);
        if start == end {
            continue;
        }
        let overlap = budget
            .overlap(
                window,
                pattern.x_um[pair.target] - pattern.x_um[pair.source],
                pattern.y_um[pair.target] - pattern.y_um[pair.source],
                pair.source,
                pair.target,
            )
            .map_err(translation_dependency)?;
        overlaps.insert((pair.source, pair.target), overlap);
    }
    Ok(overlaps)
}

#[allow(clippy::too_many_arguments)]
fn evaluate(
    codes: &[u32],
    source_code: u32,
    target_code: u32,
    source_count: usize,
    target_count: usize,
    area: f64,
    plan: &MarkPairPlan,
    overlaps: &BTreeMap<(usize, usize), f64>,
    config: &super::CategoricalCrossPairCorrelationConfig,
) -> Result<Evaluation, CategoricalCrossPairCorrelationError> {
    let mut pair_counts = vec![0_usize; config.radii_um.len()];
    let mut overlap_counts = vec![0_usize; config.radii_um.len()];
    let mut overlap_sums = vec![0.0_f64; config.radii_um.len()];
    let mut weighted_sums = vec![0.0_f64; config.radii_um.len()];
    for pair in &plan.pairs {
        if codes[pair.source] != source_code || codes[pair.target] != target_code {
            continue;
        }
        let key = (pair.source.min(pair.target), pair.source.max(pair.target));
        let Some(overlap) = overlaps.get(&key).copied() else {
            continue;
        };
        let lower = pair.distance_um - config.bandwidth_um;
        let upper = pair.distance_um + config.bandwidth_um;
        let start = config.radii_um.partition_point(|radius| *radius <= lower);
        let end = config.radii_um.partition_point(|radius| *radius < upper);
        for index in start..end {
            let kernel = epanechnikov_weight(
                config.radii_um[index],
                pair.distance_um,
                config.bandwidth_um,
            )
            .expect("strict compact support");
            pair_counts[index] += 1;
            overlap_counts[index] += 1;
            overlap_sums[index] += overlap;
            weighted_sums[index] += kernel * area / overlap;
        }
    }
    let mut points = Vec::with_capacity(config.radii_um.len());
    let mut values = Vec::with_capacity(config.radii_um.len());
    let mut eligible = Vec::with_capacity(config.radii_um.len());
    for index in 0..config.radii_um.len() {
        let available = pair_counts[index] > 0;
        let cross_g = available.then(|| {
            area * weighted_sums[index]
                / (std::f64::consts::TAU
                    * config.radii_um[index]
                    * source_count as f64
                    * target_count as f64)
        });
        if cross_g.is_some_and(|value| !value.is_finite()) {
            return Err(dependency(
                "translation cross-g normalization is non-finite",
            ));
        }
        values.push(cross_g.unwrap_or(0.0));
        eligible.push(available);
        points.push(TranslationCategoricalCrossPairCorrelationPoint {
            radius_um: config.radii_um[index],
            status: if available {
                PairCorrelationPointStatus::Available
            } else {
                PairCorrelationPointStatus::NoPairsInKernelSupport
            },
            directed_source_target_pairs_in_support: pair_counts[index],
            overlap_evaluations: overlap_counts[index],
            translation_overlap_area_sum_um2: canonical_zero(overlap_sums[index]),
            translation_weighted_kernel_sum: canonical_zero(weighted_sums[index]),
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
    overlap_count: usize,
    config: &TranslationCategoricalCrossPairCorrelationConfig,
) -> Result<usize, CategoricalCrossPairCorrelationError> {
    let base = &config.base;
    let matrices = base
        .permutations
        .checked_mul(base.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let rows = rows
        .checked_mul(4 * std::mem::size_of::<usize>())
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let overlaps = overlap_count
        .checked_mul(std::mem::size_of::<((usize, usize), f64)>())
        .and_then(|value| value.checked_mul(3))
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let erl = erl_workspace_bytes(base.permutations, base.radii_um.len())
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    plan_bytes
        .checked_add(matrices)
        .and_then(|value| value.checked_add(rows))
        .and_then(|value| value.checked_add(overlaps))
        .and_then(|value| value.checked_add(erl))
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)
}

pub(crate) fn configuration_digest(
    config: &TranslationCategoricalCrossPairCorrelationConfig,
) -> ContentDigest {
    let base = super::configuration_digest(&config.base);
    let limits = config.translation_limits;
    ContentDigest::from_framed([
        b"marklab-translation-categorical-cross-g-config-v1".as_slice(),
        base.as_bytes(),
        &(limits.maximum_points as u128).to_be_bytes(),
        &(limits.maximum_radii as u128).to_be_bytes(),
        &(limits.maximum_pair_visits as u128).to_be_bytes(),
        &(limits.maximum_overlap_evaluations as u128).to_be_bytes(),
        &(limits.maximum_overlap_candidate_work as u128).to_be_bytes(),
        &(limits.maximum_overlap_output_vertices as u128).to_be_bytes(),
        &(limits.maximum_csr_draws as u128).to_be_bytes(),
        &(limits.maximum_retained_bytes as u128).to_be_bytes(),
    ])
}

fn resolve(levels: &[String], name: &str) -> Result<u32, CategoricalCrossPairCorrelationError> {
    levels
        .iter()
        .position(|level| level == name)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| CategoricalCrossPairCorrelationError::MissingLevel(name.into()))
}

fn measurement_status_name(status: MeasurementStatus) -> &'static str {
    match status {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

fn dependency(error: impl std::fmt::Display) -> CategoricalCrossPairCorrelationError {
    CategoricalCrossPairCorrelationError::Dependency(error.to_string())
}

fn translation_dependency(error: TranslationSpatialError) -> CategoricalCrossPairCorrelationError {
    dependency(error)
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

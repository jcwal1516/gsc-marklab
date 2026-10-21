use crate::edge_corrected::{cache_corrections, estimate_cross, CrossWork};
use crate::result_vocabulary::{EdgeCorrection, MarkNullModel, MarkPermutationUnit};
use std::collections::BTreeMap;

use marklab_cohort::{InferenceAlternative, InferenceDesign};
use marklab_project::ContentDigest;
use serde::{Deserialize, Serialize};

use crate::common::finite::canonical_zero;

use crate::{
    classical::window_summary,
    isotropic_spatial::edge::IsotropicArcBudget,
    mark_pair_plan::{build_mark_pair_plan, erl_workspace_bytes, MarkPairPlan},
    marks::{DeclaredScalarPatternInput, ScalarMarkId},
    permutation::envelopes::GlobalEnvelope,
    scalar_mark::COMPARTMENT_MARK_ID,
    spatial::{
        ClassicalWindowSummary, IsotropicSpatialError, IsotropicSpatialLimits,
        PairCorrelationKernel, PairCorrelationPointStatus,
    },
    ObservationWindow2D,
};

use super::{
    analysis::resolve, CategoricalCrossPairCorrelationConfig, CategoricalCrossPairCorrelationError,
    CategoricalCrossPairCorrelationInference,
};

#[derive(Clone, Debug, PartialEq)]
pub struct IsotropicCategoricalCrossPairCorrelationConfig {
    pub(super) base: CategoricalCrossPairCorrelationConfig,
    pub(super) limits: IsotropicSpatialLimits,
}

impl IsotropicCategoricalCrossPairCorrelationConfig {
    pub fn new(
        base: CategoricalCrossPairCorrelationConfig,
        limits: IsotropicSpatialLimits,
    ) -> Result<Self, CategoricalCrossPairCorrelationError> {
        if limits.maximum_points < base.limits.maximum_points
            || limits.maximum_radii < base.radii_um.len()
            || limits.maximum_retained_bytes < base.limits.maximum_retained_bytes
        {
            return Err(CategoricalCrossPairCorrelationError::InvalidConfig(
                "isotropic ceilings must cover categorical point, radius, and memory limits".into(),
            ));
        }
        Ok(Self { base, limits })
    }

    pub fn base(&self) -> &CategoricalCrossPairCorrelationConfig {
        &self.base
    }

    pub fn isotropic_limits(&self) -> IsotropicSpatialLimits {
        self.limits
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicCategoricalCrossPairCorrelationPoint {
    pub radius_um: f64,
    pub status: PairCorrelationPointStatus,
    pub directed_source_target_pairs_in_support: usize,
    pub visible_arc_evaluations: usize,
    pub visible_arc_fraction_sum: f64,
    pub inverse_visible_arc_weighted_kernel_sum: f64,
    pub cross_g: Option<f64>,
    pub theoretical_cross_g: f64,
    pub inference_eligible: bool,
    pub lower_cross_g: Option<f64>,
    pub upper_cross_g: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicCategoricalCrossPairCorrelationResult {
    pub mark_id: String,
    #[serde(with = "crate::measurement_status_wire")]
    pub measurement_status: marklab_data::MeasurementStatus,
    pub coordinate_frame_id: String,
    pub window: ClassicalWindowSummary,
    pub source_level: String,
    pub target_level: String,
    pub source_count: usize,
    pub target_count: usize,
    pub kernel: PairCorrelationKernel,
    pub bandwidth_um: f64,
    pub edge_correction: EdgeCorrection,
    pub geometry_digest: String,
    pub pair_plan_digest: String,
    pub retained_directed_pair_count: usize,
    pub observed_unordered_pair_visits: usize,
    pub visible_arc_evaluations: usize,
    pub arc_segment_tests: usize,
    pub arc_membership_queries: usize,
    pub maximum_boundary_intersection_angles: usize,
    pub estimated_storage_bytes: usize,
    pub configuration_digest: String,
    pub curve: Vec<IsotropicCategoricalCrossPairCorrelationPoint>,
    pub inference: CategoricalCrossPairCorrelationInference,
}

struct Evaluation {
    points: Vec<IsotropicCategoricalCrossPairCorrelationPoint>,
    values: Vec<f64>,
    eligible: Vec<bool>,
}

pub fn isotropic_categorical_cross_pair_correlation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &IsotropicCategoricalCrossPairCorrelationConfig,
) -> Result<IsotropicCategoricalCrossPairCorrelationResult, CategoricalCrossPairCorrelationError> {
    let base = &config.base;
    let frame = window
        .coordinate_frame_id()
        .ok_or(CategoricalCrossPairCorrelationError::CoordinateFrameMismatch)?;
    if frame != input.coordinate_frame_id() {
        return Err(CategoricalCrossPairCorrelationError::CoordinateFrameMismatch);
    }
    let mark_id = ScalarMarkId::new(COMPARTMENT_MARK_ID).map_err(dependency)?;
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
    let support = base.radii_um[base.radii_um.len() - 1] + base.bandwidth_um;
    let plan = build_mark_pair_plan(
        input,
        window,
        &[support],
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
    let mut budget = IsotropicArcBudget::new(config.limits, window);
    let fractions = build_fractions(input, window, base, &plan, &mut budget)?;
    let estimated_storage_bytes =
        retained_bytes(codes.len(), plan.retained_bytes, fractions.len(), config)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(
            CategoricalCrossPairCorrelationError::RetainedByteLimitExceeded {
                required: estimated_storage_bytes,
                maximum: config.limits.maximum_retained_bytes,
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
        &fractions,
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
        let mut permuted = Vec::new();
        permuted
            .try_reserve_exact(indices.len())
            .map_err(|_| CategoricalCrossPairCorrelationError::AllocationFailed)?;
        permuted.extend(indices.iter().map(|index| codes[*index]));
        let evaluated = evaluate(
            &permuted,
            source_code,
            target_code,
            source_count,
            target_count,
            window.area_um2(),
            &plan,
            &fractions,
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
    Ok(IsotropicCategoricalCrossPairCorrelationResult {
        mark_id: mark_id.as_str().into(),
        measurement_status: table
            .measurement_status(&mark_id)
            .ok_or(CategoricalCrossPairCorrelationError::MissingCategoricalMark)?,
        coordinate_frame_id: frame.as_str().into(),
        window: window_summary(window.descriptor()),
        source_level: base.source_level.clone(),
        target_level: base.target_level.clone(),
        source_count,
        target_count,
        kernel: PairCorrelationKernel::Epanechnikov,
        bandwidth_um: base.bandwidth_um,
        edge_correction: EdgeCorrection::Isotropic,
        geometry_digest: plan.geometry.logical_digest().to_string(),
        pair_plan_digest: plan.digest.to_string(),
        retained_directed_pair_count: plan.pairs.len(),
        observed_unordered_pair_visits: budget.pair_visits,
        visible_arc_evaluations: budget.visible_arc_evaluations,
        arc_segment_tests: budget.arc_segment_tests,
        arc_membership_queries: budget.arc_membership_queries,
        maximum_boundary_intersection_angles: budget.maximum_intersection_angles,
        estimated_storage_bytes,
        configuration_digest: configuration_digest(config).to_string(),
        curve: observed.points,
        inference: CategoricalCrossPairCorrelationInference {
            null_model: MarkNullModel::RandomLabeling,
            permutation_unit: MarkPermutationUnit::CompleteCategoricalRow,
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

fn build_fractions(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &CategoricalCrossPairCorrelationConfig,
    plan: &MarkPairPlan,
    budget: &mut IsotropicArcBudget,
) -> Result<BTreeMap<(usize, usize), f64>, CategoricalCrossPairCorrelationError> {
    cache_corrections(input, window, config, plan, budget).map_err(isotropic_dependency)
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
    fractions: &BTreeMap<(usize, usize), f64>,
    config: &CategoricalCrossPairCorrelationConfig,
) -> Result<Evaluation, CategoricalCrossPairCorrelationError> {
    let CrossWork {
        counts,
        measure_sums: fraction_sums,
        weighted_sums,
    } = estimate_cross::<IsotropicArcBudget>(
        codes,
        (source_code, target_code),
        area,
        plan,
        fractions,
        config,
    )?;
    let mut points = Vec::new();
    let mut values = Vec::new();
    let mut eligible = Vec::new();
    points
        .try_reserve_exact(config.radii_um.len())
        .and_then(|()| values.try_reserve_exact(config.radii_um.len()))
        .and_then(|()| eligible.try_reserve_exact(config.radii_um.len()))
        .map_err(|_| CategoricalCrossPairCorrelationError::AllocationFailed)?;
    for index in 0..config.radii_um.len() {
        let has_pairs = counts[index] > 0;
        let cross_g = (source_count > 0 && target_count > 0).then(|| {
            area * weighted_sums[index]
                / (std::f64::consts::TAU
                    * config.radii_um[index]
                    * source_count as f64
                    * target_count as f64)
        });
        if cross_g.is_some_and(|value| !value.is_finite()) {
            return Err(dependency("isotropic cross-g normalization is non-finite"));
        }
        values.push(cross_g.unwrap_or(0.0));
        eligible.push(cross_g.is_some());
        points.push(IsotropicCategoricalCrossPairCorrelationPoint {
            radius_um: config.radii_um[index],
            status: if has_pairs {
                PairCorrelationPointStatus::Available
            } else {
                PairCorrelationPointStatus::NoPairsInKernelSupport
            },
            directed_source_target_pairs_in_support: counts[index],
            visible_arc_evaluations: counts[index],
            visible_arc_fraction_sum: canonical_zero(fraction_sums[index]),
            inverse_visible_arc_weighted_kernel_sum: canonical_zero(weighted_sums[index]),
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
    fractions: usize,
    config: &IsotropicCategoricalCrossPairCorrelationConfig,
) -> Result<usize, CategoricalCrossPairCorrelationError> {
    let base = &config.base;
    let matrices = base
        .permutations
        .checked_mul(base.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let row_bytes = rows
        .checked_mul(4 * std::mem::size_of::<usize>())
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let fraction_bytes = fractions
        .checked_mul(std::mem::size_of::<((usize, usize), f64)>())
        .and_then(|value| value.checked_mul(3))
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    let erl = erl_workspace_bytes(base.permutations, base.radii_um.len())
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
    plan_bytes
        .checked_add(matrices)
        .and_then(|value| value.checked_add(row_bytes))
        .and_then(|value| value.checked_add(fraction_bytes))
        .and_then(|value| value.checked_add(erl))
        .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)
}

pub(crate) fn configuration_digest(
    config: &IsotropicCategoricalCrossPairCorrelationConfig,
) -> ContentDigest {
    let base = super::configuration_digest(&config.base);
    let limits = config.limits;
    ContentDigest::from_framed([
        b"marklab-isotropic-categorical-cross-g-config-v1".as_slice(),
        base.as_bytes(),
        &(limits.maximum_points as u128).to_be_bytes(),
        &(limits.maximum_radii as u128).to_be_bytes(),
        &(limits.maximum_pair_visits as u128).to_be_bytes(),
        &(limits.maximum_visible_arc_evaluations as u128).to_be_bytes(),
        &(limits.maximum_arc_segment_tests as u128).to_be_bytes(),
        &(limits.maximum_arc_membership_queries as u128).to_be_bytes(),
        &(limits.maximum_csr_draws as u128).to_be_bytes(),
        &(limits.maximum_retained_bytes as u128).to_be_bytes(),
    ])
}

fn dependency(error: impl std::fmt::Display) -> CategoricalCrossPairCorrelationError {
    CategoricalCrossPairCorrelationError::Dependency(error.to_string())
}
fn isotropic_dependency(error: IsotropicSpatialError) -> CategoricalCrossPairCorrelationError {
    dependency(error)
}

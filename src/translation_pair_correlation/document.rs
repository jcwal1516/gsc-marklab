use serde::{Deserialize, Serialize};

use crate::{
    errors::{MarklabError, Result},
    ClassicalNullModel, ClassicalRandomizationUnit, PairCorrelationKernel,
    PairCorrelationPointStatus, TranslationPairCorrelationConfig, TranslationPairCorrelationResult,
    TranslationWorkflowIdentity,
};

use super::analysis::translation_pair_correlation_configuration_digest;

/// Stable format identifier for exact translation-corrected pair correlation.
pub const TRANSLATION_PAIR_CORRELATION_FORMAT: &str = "marklab.translation_pair_correlation";
/// Current standalone translation pair-correlation result version.
pub const TRANSLATION_PAIR_CORRELATION_FORMAT_VERSION: &str = "1";

/// Strict standalone document for one translation-corrected g analysis.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationPairCorrelationResultDocument {
    format: String,
    format_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workflow: Option<TranslationWorkflowIdentity>,
    analysis: TranslationPairCorrelationResult,
}

impl TranslationPairCorrelationResultDocument {
    /// Wrap and semantically validate a typed result.
    pub fn new(analysis: TranslationPairCorrelationResult) -> Result<Self> {
        let document = Self {
            format: TRANSLATION_PAIR_CORRELATION_FORMAT.into(),
            format_version: TRANSLATION_PAIR_CORRELATION_FORMAT_VERSION.into(),
            workflow: None,
            analysis,
        };
        document.validate()?;
        Ok(document)
    }

    /// Decode and semantically validate strict version-one JSON.
    pub fn from_json(text: &str) -> Result<Self> {
        let document: Self = serde_json::from_str(text).map_err(|error| {
            MarklabError::Schema(format!(
                "invalid translation pair-correlation result JSON: {error}"
            ))
        })?;
        document.validate()?;
        Ok(document)
    }

    /// Borrow the typed analysis.
    pub fn analysis(&self) -> &TranslationPairCorrelationResult {
        &self.analysis
    }

    /// Consume the document and return its typed analysis.
    pub fn into_analysis(self) -> TranslationPairCorrelationResult {
        self.analysis
    }

    /// Attach exact durable workflow identity.
    pub fn with_workflow_identity(mut self, workflow: TranslationWorkflowIdentity) -> Result<Self> {
        self.workflow = Some(workflow);
        self.validate()?;
        Ok(self)
    }

    /// Borrow durable workflow identity when present.
    pub fn workflow(&self) -> Option<&TranslationWorkflowIdentity> {
        self.workflow.as_ref()
    }

    /// Encode deterministic human-readable JSON.
    pub fn to_json_pretty(&self) -> Result<String> {
        self.validate()?;
        stable_pretty_json(self)
    }

    fn validate(&self) -> Result<()> {
        if self.format != TRANSLATION_PAIR_CORRELATION_FORMAT {
            return schema("translation pair-correlation format differs");
        }
        if self.format_version != TRANSLATION_PAIR_CORRELATION_FORMAT_VERSION {
            return Err(MarklabError::UnsupportedFormatVersion {
                found: self.format_version.clone(),
                supported: TRANSLATION_PAIR_CORRELATION_FORMAT_VERSION.into(),
            });
        }
        if self.workflow.as_ref().is_some_and(|workflow| {
            !valid_digest(&workflow.cache_key)
                || !valid_digest(&workflow.output_artifact_digest)
                || workflow.output_artifact_bytes == 0
        }) {
            return schema("translation pair-correlation workflow identity is invalid");
        }
        validate_analysis(&self.analysis)
    }
}

fn stable_pretty_json(document: &TranslationPairCorrelationResultDocument) -> Result<String> {
    let mut encoded = serde_json::to_string_pretty(document).map_err(|error| {
        MarklabError::Compute(format!(
            "translation pair-correlation result encoding failed: {error}"
        ))
    })?;
    for _ in 0..3 {
        let decoded: TranslationPairCorrelationResultDocument = serde_json::from_str(&encoded)
            .map_err(|error| {
                MarklabError::Compute(format!(
                    "translation pair-correlation normalization failed: {error}"
                ))
            })?;
        let next = serde_json::to_string_pretty(&decoded).map_err(|error| {
            MarklabError::Compute(format!(
                "translation pair-correlation result encoding failed: {error}"
            ))
        })?;
        if next == encoded {
            return Ok(encoded);
        }
        encoded = next;
    }
    Err(MarklabError::Compute(
        "translation pair-correlation JSON did not reach a stable numeric representation".into(),
    ))
}

fn validate_analysis(analysis: &TranslationPairCorrelationResult) -> Result<()> {
    let window = &analysis.window;
    if analysis.correction != "translation"
        || analysis.kernel != PairCorrelationKernel::Epanechnikov
        || !analysis.bandwidth_um.is_finite()
        || analysis.bandwidth_um <= 0.0
        || analysis.randomization_unit != "whole_location_pattern"
        || window.coordinate_unit != "micrometre"
        || window.coordinate_frame != "existing_pattern_physical_xy"
        || window.membership_policy != "closed_window"
        || !window.area_um2.is_finite()
        || window.area_um2 <= 0.0
        || !window.perimeter_um.is_finite()
        || window.perimeter_um <= 0.0
        || window.bounds_um.iter().any(|value| !value.is_finite())
        || window.bounds_um[0] >= window.bounds_um[2]
        || window.bounds_um[1] >= window.bounds_um[3]
        || window.component_count == 0
        || window.ring_count != window.component_count.saturating_add(window.hole_count)
        || window.vertex_count < window.ring_count.saturating_mul(4)
        || !valid_digest(&window.logical_digest)
    {
        return schema("translation pair-correlation window or estimator identity is invalid");
    }
    let geometry = &analysis.geometry;
    if geometry.point_count != analysis.null_design.conditioned_point_count
        || !geometry.maximum_query_radius_um.is_finite()
        || geometry.maximum_query_radius_um <= analysis.bandwidth_um
        || geometry.total_pair_visits < geometry.observed_pair_visits
        || geometry.total_overlap_evaluations < geometry.observed_overlap_evaluations
        || geometry.total_overlap_evaluations > geometry.total_pair_visits
        || !valid_digest(&geometry.logical_digest)
        || geometry.execution_mode != "exact"
        || geometry.duplicate_policy != "reject"
        || geometry.overlap_owner != "observation_window_2d_exact_boolean_intersection"
        || geometry.pair_traversal
            != "exact_streaming_unordered_compact_support_pairs_with_both_ordered_weights"
    {
        return schema("translation pair-correlation geometry summary is invalid");
    }
    let configuration = &analysis.configuration;
    let limits = configuration.limits;
    if analysis.curve.is_empty()
        || configuration.radius_count != analysis.curve.len()
        || !valid_digest(&configuration.logical_digest)
        || [
            limits.maximum_points,
            limits.maximum_radii,
            limits.maximum_pair_visits,
            limits.maximum_overlap_evaluations,
            limits.maximum_overlap_candidate_work,
            limits.maximum_overlap_output_vertices,
            limits.maximum_csr_draws,
            limits.maximum_retained_bytes,
        ]
        .contains(&0)
        || limits.maximum_points < geometry.point_count
        || limits.maximum_radii < analysis.curve.len()
        || limits.maximum_pair_visits < geometry.total_pair_visits
        || limits.maximum_overlap_evaluations < geometry.total_overlap_evaluations
        || limits.maximum_overlap_candidate_work < geometry.total_overlap_candidate_work
        || limits.maximum_overlap_output_vertices
            < geometry.maximum_overlap_output_vertices_observed
        || limits.maximum_retained_bytes < geometry.estimated_storage_bytes
        || limits.maximum_csr_draws < analysis.csr_candidate_draws
    {
        return schema("translation pair-correlation telemetry exceeds its limits");
    }
    let segments = window.vertex_count.saturating_sub(window.ring_count);
    let expected_candidate_work = segments
        .checked_mul(segments)
        .and_then(|value| value.checked_mul(geometry.total_overlap_evaluations))
        .ok_or_else(|| MarklabError::Schema("translation g work count overflow".into()))?;
    if expected_candidate_work != geometry.total_overlap_candidate_work {
        return schema("translation pair-correlation candidate-work telemetry differs");
    }
    let null = &analysis.null_design;
    if null.null_model != ClassicalNullModel::HomogeneousCsrConditionalOnCount
        || null.randomization_unit != ClassicalRandomizationUnit::WholeLocationPattern
        || null.simulations == 0
        || !null.alpha.is_finite()
        || null.alpha <= 0.0
        || null.alpha >= 1.0
        || (null.simulations.saturating_add(1) as f64) * null.alpha < 1.0
    {
        return schema("translation pair-correlation null design is invalid");
    }
    let denominator = geometry
        .point_count
        .checked_mul(geometry.point_count.saturating_sub(1))
        .ok_or_else(|| MarklabError::Schema("translation g denominator overflow".into()))?;
    let unordered_pairs = denominator / 2;
    let expected_total_pair_visits = unordered_pairs
        .checked_mul(null.simulations.saturating_add(1))
        .ok_or_else(|| MarklabError::Schema("translation g total pair count overflow".into()))?;
    if geometry.observed_pair_visits != unordered_pairs
        || geometry.total_pair_visits != expected_total_pair_visits
    {
        return schema("translation pair-correlation pair-visit telemetry differs");
    }
    let mut previous_radius = 0.0;
    let mut maximum_radius_overlap_evaluations = 0_usize;
    let mut summed_radius_overlap_evaluations = 0_usize;
    for point in &analysis.curve {
        let available = point.status == PairCorrelationPointStatus::Available;
        if !point.radius_um.is_finite()
            || point.radius_um <= previous_radius
            || point.radius_um <= analysis.bandwidth_um
            || point.ordered_pairs_in_support % 2 != 0
            || point.overlap_evaluations != point.ordered_pairs_in_support / 2
            || point.ordered_pairs_in_support > denominator
            || point.status == PairCorrelationPointStatus::NoEligibleCenters
            || available != (point.ordered_pairs_in_support > 0)
            || !point.translation_overlap_area_sum_um2.is_finite()
            || point.translation_overlap_area_sum_um2 < 0.0
            || !point.translation_weighted_kernel_sum.is_finite()
            || point.translation_weighted_kernel_sum < 0.0
            || (point.overlap_evaluations == 0) != (point.translation_overlap_area_sum_um2 == 0.0)
            || (point.overlap_evaluations == 0) != (point.translation_weighted_kernel_sum == 0.0)
            || point.g.is_some() != available
            || point.theoretical_g != 1.0
            || point.inference_eligible != (point.lower_g.is_some() && point.upper_g.is_some())
            || point
                .lower_g
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || point
                .upper_g
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || point
                .lower_g
                .zip(point.upper_g)
                .is_some_and(|(lower, upper)| lower > upper)
        {
            return schema("translation pair-correlation curve identity is invalid");
        }
        if let Some(g) = point.g {
            let expected = window.area_um2 * point.translation_weighted_kernel_sum
                / (std::f64::consts::TAU * point.radius_um * denominator as f64);
            if denominator == 0 || !g.is_finite() || !same_calculated_float(g, expected) {
                return schema("translation pair-correlation normalization differs");
            }
        }
        maximum_radius_overlap_evaluations =
            maximum_radius_overlap_evaluations.max(point.overlap_evaluations);
        summed_radius_overlap_evaluations = summed_radius_overlap_evaluations
            .checked_add(point.overlap_evaluations)
            .ok_or_else(|| MarklabError::Schema("translation g overlap count overflow".into()))?;
        previous_radius = point.radius_um;
    }
    if geometry.observed_overlap_evaluations < maximum_radius_overlap_evaluations
        || geometry.observed_overlap_evaluations > summed_radius_overlap_evaluations
    {
        return schema("translation pair-correlation observed overlap telemetry differs");
    }
    if analysis.curve.last().is_none_or(|point| {
        !same_calculated_float(
            point.radius_um + analysis.bandwidth_um,
            geometry.maximum_query_radius_um,
        )
    }) {
        return schema("translation pair-correlation maximum query radius differs");
    }
    let config = TranslationPairCorrelationConfig::new(
        analysis.curve.iter().map(|point| point.radius_um).collect(),
        analysis.bandwidth_um,
        null.simulations,
        null.seed,
        null.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Schema(error.to_string()))?;
    if configuration.logical_digest
        != translation_pair_correlation_configuration_digest(&config).to_string()
    {
        return schema("translation pair-correlation configuration digest differs");
    }
    match &analysis.inference {
        Some(inference) => {
            let eligible_count = analysis
                .curve
                .iter()
                .filter(|point| point.inference_eligible)
                .count();
            if geometry.point_count < 2
                || eligible_count == 0
                || inference.simulations != null.simulations
                || inference.eligible_radius_count != eligible_count
                || !unit_interval(inference.p_global)
                || inference.p_global < 1.0 / (null.simulations + 1) as f64
                || !unit_interval(inference.erl_depth)
                || !unit_interval(inference.critical_depth)
            {
                return schema("translation pair-correlation inference summary differs");
            }
        }
        None => {
            if analysis.curve.iter().any(|point| point.inference_eligible) {
                return schema("translation pair-correlation inference eligibility differs");
            }
            if geometry.point_count < 2
                && (analysis.csr_candidate_draws != 0
                    || geometry.total_pair_visits != 0
                    || geometry.total_overlap_evaluations != 0
                    || geometry.total_overlap_candidate_work != 0)
            {
                return schema("insufficient translation pair-correlation result is inconsistent");
            }
        }
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn same_calculated_float(left: f64, right: f64) -> bool {
    left == right
        || (left.is_finite() && right.is_finite() && left.to_bits().abs_diff(right.to_bits()) <= 1)
}

fn schema<T>(reason: &str) -> Result<T> {
    Err(MarklabError::Schema(reason.into()))
}

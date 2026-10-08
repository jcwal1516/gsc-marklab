use serde::{Deserialize, Serialize};

use crate::{
    errors::{MarklabError, Result},
    ClassicalNullModel, ClassicalRandomizationUnit, IsotropicPairCorrelationConfig,
    IsotropicPairCorrelationResult, IsotropicWorkflowIdentity, PairCorrelationKernel,
    PairCorrelationPointStatus,
};

use super::analysis::isotropic_pair_correlation_configuration_digest;

/// Stable standalone format identifier.
pub const ISOTROPIC_PAIR_CORRELATION_FORMAT: &str = "marklab.isotropic_pair_correlation";
/// Current standalone format version.
pub const ISOTROPIC_PAIR_CORRELATION_FORMAT_VERSION: &str = "1";

/// Strict standalone document for exact isotropic homogeneous g.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicPairCorrelationResultDocument {
    format: String,
    format_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workflow: Option<IsotropicWorkflowIdentity>,
    analysis: IsotropicPairCorrelationResult,
}

impl IsotropicPairCorrelationResultDocument {
    /// Wrap and semantically validate a typed result.
    pub fn new(analysis: IsotropicPairCorrelationResult) -> Result<Self> {
        let document = Self {
            format: ISOTROPIC_PAIR_CORRELATION_FORMAT.into(),
            format_version: ISOTROPIC_PAIR_CORRELATION_FORMAT_VERSION.into(),
            workflow: None,
            analysis,
        };
        document.validate()?;
        Ok(document)
    }
    /// Decode and semantically validate strict version-one JSON.
    pub fn from_json(text: &str) -> Result<Self> {
        let document: Self = serde_json::from_str(text).map_err(|error| {
            MarklabError::Schema(format!("invalid isotropic pair-correlation JSON: {error}"))
        })?;
        document.validate()?;
        Ok(document)
    }
    /// Borrow the typed analysis.
    pub fn analysis(&self) -> &IsotropicPairCorrelationResult {
        &self.analysis
    }
    /// Consume the document and return the typed analysis.
    pub fn into_analysis(self) -> IsotropicPairCorrelationResult {
        self.analysis
    }
    /// Attach exact durable workflow identity.
    pub fn with_workflow_identity(mut self, workflow: IsotropicWorkflowIdentity) -> Result<Self> {
        self.workflow = Some(workflow);
        self.validate()?;
        Ok(self)
    }
    /// Borrow durable workflow identity when present.
    pub fn workflow(&self) -> Option<&IsotropicWorkflowIdentity> {
        self.workflow.as_ref()
    }
    /// Encode deterministic human-readable JSON.
    pub fn to_json_pretty(&self) -> Result<String> {
        self.validate()?;
        stable_json(self)
    }

    fn validate(&self) -> Result<()> {
        if self.format != ISOTROPIC_PAIR_CORRELATION_FORMAT {
            return schema("format differs");
        }
        if self.format_version != ISOTROPIC_PAIR_CORRELATION_FORMAT_VERSION {
            return Err(MarklabError::UnsupportedFormatVersion {
                found: self.format_version.clone(),
                supported: ISOTROPIC_PAIR_CORRELATION_FORMAT_VERSION.into(),
            });
        }
        if self.workflow.as_ref().is_some_and(|workflow| {
            !valid_digest(&workflow.cache_key)
                || !valid_digest(&workflow.output_artifact_digest)
                || workflow.output_artifact_bytes == 0
        }) {
            return schema("workflow identity invalid");
        }
        validate_analysis(&self.analysis)
    }
}

fn stable_json(document: &IsotropicPairCorrelationResultDocument) -> Result<String> {
    let mut encoded = serde_json::to_string_pretty(document)
        .map_err(|error| MarklabError::Compute(format!("isotropic g encoding failed: {error}")))?;
    for _ in 0..3 {
        let decoded: IsotropicPairCorrelationResultDocument = serde_json::from_str(&encoded)
            .map_err(|error| {
                MarklabError::Compute(format!("isotropic g normalization failed: {error}"))
            })?;
        let next = serde_json::to_string_pretty(&decoded).map_err(|error| {
            MarklabError::Compute(format!("isotropic g encoding failed: {error}"))
        })?;
        if next == encoded {
            return Ok(encoded);
        }
        encoded = next;
    }
    Err(MarklabError::Compute(
        "isotropic g JSON did not reach a stable numeric representation".into(),
    ))
}

fn validate_analysis(analysis: &IsotropicPairCorrelationResult) -> Result<()> {
    let window = &analysis.window;
    let geometry = &analysis.geometry;
    let limits = analysis.configuration.limits;
    let null = &analysis.null_design;
    if analysis.correction != "isotropic"
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
        || window.component_count == 0
        || !valid_digest(&window.logical_digest)
        || geometry.point_count != null.conditioned_point_count
        || !geometry.maximum_query_radius_um.is_finite()
        || geometry.total_pair_visits < geometry.observed_pair_visits
        || geometry.total_visible_arc_evaluations < geometry.observed_visible_arc_evaluations
        || geometry.total_visible_arc_evaluations > geometry.total_pair_visits.saturating_mul(2)
        || geometry.total_arc_segment_tests
            != geometry
                .total_visible_arc_evaluations
                .saturating_mul(window.vertex_count.saturating_sub(window.ring_count))
        || !valid_digest(&geometry.logical_digest)
        || geometry.execution_mode != "exact"
        || geometry.duplicate_policy != "reject"
        || geometry.visible_arc_owner != "observation_window_2d_segment_circle_partition"
        || geometry.pair_traversal
            != "exact_streaming_unordered_compact_support_pairs_with_two_directed_arc_weights"
        || analysis.curve.is_empty()
        || analysis.configuration.radius_count != analysis.curve.len()
        || !valid_digest(&analysis.configuration.logical_digest)
        || limits.maximum_points < geometry.point_count
        || limits.maximum_radii < analysis.curve.len()
        || limits.maximum_pair_visits < geometry.total_pair_visits
        || limits.maximum_visible_arc_evaluations < geometry.total_visible_arc_evaluations
        || limits.maximum_arc_segment_tests < geometry.total_arc_segment_tests
        || limits.maximum_arc_membership_queries < geometry.total_arc_membership_queries
        || limits.maximum_csr_draws < analysis.csr_candidate_draws
        || limits.maximum_retained_bytes < geometry.estimated_storage_bytes
        || null.null_model != ClassicalNullModel::HomogeneousCsrConditionalOnCount
        || null.randomization_unit != ClassicalRandomizationUnit::WholeLocationPattern
        || null.simulations == 0
        || !null.alpha.is_finite()
        || null.alpha <= 0.0
        || null.alpha >= 1.0
    {
        return schema("isotropic pair-correlation identity or telemetry invalid");
    }
    let denominator = geometry
        .point_count
        .checked_mul(geometry.point_count.saturating_sub(1))
        .ok_or_else(|| MarklabError::Schema("denominator overflow".into()))?;
    let expected_pairs = (denominator / 2)
        .checked_mul(null.simulations.saturating_add(1))
        .ok_or_else(|| MarklabError::Schema("pair count overflow".into()))?;
    if geometry.observed_pair_visits != denominator / 2
        || geometry.total_pair_visits != expected_pairs
    {
        return schema("pair telemetry differs");
    }
    let mut previous = 0.0;
    let mut maximum_arc = 0_usize;
    let mut summed_arc = 0_usize;
    for point in &analysis.curve {
        let available = point.status == PairCorrelationPointStatus::Available;
        if !point.radius_um.is_finite()
            || point.radius_um <= previous
            || point.radius_um <= analysis.bandwidth_um
            || point.status == PairCorrelationPointStatus::NoEligibleCenters
            || point.directed_pairs_in_support != point.visible_arc_evaluations
            || point.directed_pairs_in_support % 2 != 0
            || point.directed_pairs_in_support > denominator
            || available != (point.directed_pairs_in_support > 0)
            || !point.visible_arc_fraction_sum.is_finite()
            || point.visible_arc_fraction_sum < 0.0
            || !point.inverse_visible_arc_weighted_kernel_sum.is_finite()
            || point.inverse_visible_arc_weighted_kernel_sum < 0.0
            || (point.directed_pairs_in_support == 0) != (point.visible_arc_fraction_sum == 0.0)
            || (point.directed_pairs_in_support == 0)
                != (point.inverse_visible_arc_weighted_kernel_sum == 0.0)
            || point.g.is_some() != available
            || point.theoretical_g != 1.0
            || point.inference_eligible != (point.lower_g.is_some() && point.upper_g.is_some())
            || point
                .lower_g
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || point
                .upper_g
                .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return schema("curve identity invalid");
        }
        if let Some(g) = point.g {
            let expected = window.area_um2 * point.inverse_visible_arc_weighted_kernel_sum
                / (std::f64::consts::TAU * point.radius_um * denominator as f64);
            if denominator == 0 || !g.is_finite() || !same_float(g, expected) {
                return schema("g normalization differs");
            }
        }
        maximum_arc = maximum_arc.max(point.visible_arc_evaluations);
        summed_arc = summed_arc
            .checked_add(point.visible_arc_evaluations)
            .ok_or_else(|| MarklabError::Schema("arc count overflow".into()))?;
        previous = point.radius_um;
    }
    if geometry.observed_visible_arc_evaluations < maximum_arc
        || geometry.observed_visible_arc_evaluations > summed_arc
    {
        return schema("observed arc telemetry differs");
    }
    if analysis.curve.last().is_none_or(|point| {
        !same_float(
            point.radius_um + analysis.bandwidth_um,
            geometry.maximum_query_radius_um,
        )
    }) {
        return schema("maximum query radius differs");
    }
    let config = IsotropicPairCorrelationConfig::new(
        analysis.curve.iter().map(|p| p.radius_um).collect(),
        analysis.bandwidth_um,
        null.simulations,
        null.seed,
        null.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Schema(error.to_string()))?;
    if analysis.configuration.logical_digest
        != isotropic_pair_correlation_configuration_digest(&config).to_string()
    {
        return schema("configuration digest differs");
    }
    let eligible = analysis
        .curve
        .iter()
        .filter(|point| point.inference_eligible)
        .count();
    match &analysis.inference {
        Some(inference)
            if eligible > 0
                && inference.simulations == null.simulations
                && inference.eligible_radius_count == eligible
                && unit(inference.p_global)
                && unit(inference.erl_depth)
                && unit(inference.critical_depth) => {}
        None if eligible == 0 => {}
        _ => return schema("inference differs"),
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
fn same_float(left: f64, right: f64) -> bool {
    left == right
        || (left.is_finite() && right.is_finite() && left.to_bits().abs_diff(right.to_bits()) <= 1)
}
fn schema<T>(reason: &str) -> Result<T> {
    Err(MarklabError::Schema(reason.into()))
}

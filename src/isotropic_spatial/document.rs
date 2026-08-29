use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};

use super::{
    analysis::isotropic_configuration_digest, IsotropicSpatialConfig, IsotropicSpatialResult,
    IsotropicSpatialStatus,
};

/// Stable format identifier for isotropic spatial results.
pub const ISOTROPIC_SPATIAL_FORMAT: &str = "marklab.isotropic_spatial";
/// Current standalone isotropic result version.
pub const ISOTROPIC_SPATIAL_FORMAT_VERSION: &str = "1";

/// Strict standalone document for one isotropic K/L analysis.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicSpatialResultDocument {
    format: String,
    format_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workflow: Option<IsotropicWorkflowIdentity>,
    analysis: IsotropicSpatialResult,
}

/// Durable cache disposition attached by the project CLI.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IsotropicCacheStatus {
    /// The node executed and committed canonical output.
    Miss,
    /// Verified canonical output was replayed.
    Hit,
}

/// Exact scheduler/cache identity attached to a standalone result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsotropicWorkflowIdentity {
    /// Scheduler cache key.
    pub cache_key: String,
    /// Hit or miss disposition.
    pub cache_status: IsotropicCacheStatus,
    /// Canonical node-output artifact digest.
    pub output_artifact_digest: String,
    /// Exact node-output bytes.
    pub output_artifact_bytes: u64,
}

impl IsotropicSpatialResultDocument {
    /// Wrap and validate one typed isotropic analysis.
    pub fn new(analysis: IsotropicSpatialResult) -> Result<Self> {
        let document = Self {
            format: ISOTROPIC_SPATIAL_FORMAT.into(),
            format_version: ISOTROPIC_SPATIAL_FORMAT_VERSION.into(),
            workflow: None,
            analysis,
        };
        document.validate()?;
        Ok(document)
    }

    /// Decode and semantically validate strict version-one JSON.
    pub fn from_json(text: &str) -> Result<Self> {
        let document: Self = serde_json::from_str(text).map_err(|error| {
            MarklabError::Schema(format!("invalid isotropic spatial result JSON: {error}"))
        })?;
        document.validate()?;
        Ok(document)
    }

    /// Borrow the typed analysis.
    pub fn analysis(&self) -> &IsotropicSpatialResult {
        &self.analysis
    }

    /// Consume the wrapper and return the typed analysis.
    pub fn into_analysis(self) -> IsotropicSpatialResult {
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
        stable_pretty_json(self)
    }

    fn validate(&self) -> Result<()> {
        if self.format != ISOTROPIC_SPATIAL_FORMAT {
            return schema("isotropic result format differs");
        }
        if self.format_version != ISOTROPIC_SPATIAL_FORMAT_VERSION {
            return Err(MarklabError::UnsupportedFormatVersion {
                found: self.format_version.clone(),
                supported: ISOTROPIC_SPATIAL_FORMAT_VERSION.into(),
            });
        }
        if self.workflow.as_ref().is_some_and(|workflow| {
            !valid_digest(&workflow.cache_key)
                || !valid_digest(&workflow.output_artifact_digest)
                || workflow.output_artifact_bytes == 0
        }) {
            return schema("isotropic workflow identity is invalid");
        }
        validate_analysis(&self.analysis)
    }
}

fn stable_pretty_json(document: &IsotropicSpatialResultDocument) -> Result<String> {
    let mut encoded = serde_json::to_string_pretty(document).map_err(|error| {
        MarklabError::Compute(format!("isotropic result encoding failed: {error}"))
    })?;
    for _ in 0..3 {
        let decoded: IsotropicSpatialResultDocument =
            serde_json::from_str(&encoded).map_err(|error| {
                MarklabError::Compute(format!("isotropic normalization failed: {error}"))
            })?;
        let next = serde_json::to_string_pretty(&decoded).map_err(|error| {
            MarklabError::Compute(format!("isotropic result encoding failed: {error}"))
        })?;
        if next == encoded {
            return Ok(encoded);
        }
        encoded = next;
    }
    Err(MarklabError::Compute(
        "isotropic result JSON did not reach a stable numeric representation".into(),
    ))
}

fn validate_analysis(analysis: &IsotropicSpatialResult) -> Result<()> {
    let window = &analysis.window;
    if analysis.correction != "isotropic"
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
        return schema("isotropic result window or correction is invalid");
    }
    let geometry = &analysis.geometry;
    if geometry.point_count != analysis.null_design.conditioned_point_count
        || !geometry.maximum_radius_um.is_finite()
        || geometry.maximum_radius_um <= 0.0
        || geometry.total_pair_visits < geometry.observed_pair_visits
        || geometry.total_visible_arc_evaluations < geometry.observed_visible_arc_evaluations
        || geometry.total_visible_arc_evaluations > geometry.total_pair_visits.saturating_mul(2)
        || !valid_digest(&geometry.logical_digest)
        || geometry.execution_mode != "exact"
        || geometry.duplicate_policy != "reject"
        || geometry.visible_arc_owner != "observation_window_2d_segment_circle_partition"
        || geometry.pair_traversal
            != "exact_streaming_unordered_pairs_with_two_directed_arc_weights"
    {
        return schema("isotropic result geometry summary is invalid");
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
            limits.maximum_visible_arc_evaluations,
            limits.maximum_arc_segment_tests,
            limits.maximum_arc_membership_queries,
            limits.maximum_csr_draws,
            limits.maximum_retained_bytes,
        ]
        .contains(&0)
        || limits.maximum_points < geometry.point_count
        || limits.maximum_radii < analysis.curve.len()
        || limits.maximum_pair_visits < geometry.total_pair_visits
        || limits.maximum_visible_arc_evaluations < geometry.total_visible_arc_evaluations
        || limits.maximum_arc_segment_tests < geometry.total_arc_segment_tests
        || limits.maximum_arc_membership_queries < geometry.total_arc_membership_queries
        || limits.maximum_csr_draws < analysis.csr_candidate_draws
        || limits.maximum_retained_bytes < geometry.estimated_storage_bytes
    {
        return schema("isotropic result configuration or telemetry exceeds its limits");
    }
    let segments = window.vertex_count.saturating_sub(window.ring_count);
    let expected_segment_tests = segments
        .checked_mul(geometry.total_visible_arc_evaluations)
        .ok_or_else(|| MarklabError::Schema("isotropic segment work overflow".into()))?;
    if geometry.total_arc_segment_tests != expected_segment_tests
        || geometry.maximum_boundary_intersection_angles > segments.saturating_mul(2)
    {
        return schema("isotropic angular-work telemetry differs");
    }
    let null = &analysis.null_design;
    if null.simulations == 0
        || !null.alpha.is_finite()
        || null.alpha <= 0.0
        || null.alpha >= 1.0
        || (null.simulations.saturating_add(1) as f64) * null.alpha < 1.0
    {
        return schema("isotropic null design is invalid");
    }
    let denominator = geometry
        .point_count
        .checked_mul(geometry.point_count.saturating_sub(1))
        .ok_or_else(|| MarklabError::Schema("isotropic denominator overflow".into()))?;
    let unordered_pairs = denominator / 2;
    let expected_total_pair_visits = unordered_pairs
        .checked_mul(null.simulations.saturating_add(1))
        .ok_or_else(|| MarklabError::Schema("isotropic total pair count overflow".into()))?;
    if geometry.observed_pair_visits != unordered_pairs
        || geometry.total_pair_visits != expected_total_pair_visits
    {
        return schema("isotropic pair-visit telemetry differs");
    }
    let mut previous_radius = 0.0;
    let mut previous_pairs = 0_usize;
    let mut previous_fraction_sum = 0.0_f64;
    let mut previous_inverse_sum = 0.0_f64;
    for point in &analysis.curve {
        if !point.radius_um.is_finite()
            || point.radius_um <= previous_radius
            || !same_calculated_float(
                point.theoretical_k,
                std::f64::consts::PI * point.radius_um * point.radius_um,
            )
            || point.theoretical_l != point.radius_um
            || point.directed_pairs != point.visible_arc_evaluations
            || point.directed_pairs % 2 != 0
            || point.directed_pairs > denominator
            || !point.visible_arc_fraction_sum.is_finite()
            || point.visible_arc_fraction_sum < 0.0
            || point.visible_arc_fraction_sum > point.directed_pairs as f64
            || !point.inverse_visible_arc_fraction_sum.is_finite()
            || point.inverse_visible_arc_fraction_sum < point.directed_pairs as f64
            || (point.directed_pairs == 0) != (point.visible_arc_fraction_sum == 0.0)
            || (point.directed_pairs == 0) != (point.inverse_visible_arc_fraction_sum == 0.0)
            || point.directed_pairs < previous_pairs
            || point.visible_arc_fraction_sum < previous_fraction_sum
            || point.inverse_visible_arc_fraction_sum < previous_inverse_sum
        {
            return schema("isotropic curve identity or count is invalid");
        }
        previous_radius = point.radius_um;
        previous_pairs = point.directed_pairs;
        previous_fraction_sum = point.visible_arc_fraction_sum;
        previous_inverse_sum = point.inverse_visible_arc_fraction_sum;
        match (analysis.status, point.k, point.l) {
            (IsotropicSpatialStatus::Available, Some(k), Some(l)) => {
                let expected_k =
                    window.area_um2 * point.inverse_visible_arc_fraction_sum / denominator as f64;
                if denominator == 0
                    || !k.is_finite()
                    || k < 0.0
                    || !l.is_finite()
                    || l < 0.0
                    || !same_calculated_float(k, expected_k)
                    || !same_calculated_float(l, (k / std::f64::consts::PI).sqrt())
                    || !finite_nonnegative(point.lower_l)
                    || !finite_nonnegative(point.upper_l)
                    || point.lower_l > point.upper_l
                {
                    return schema("available isotropic K/L identity is invalid");
                }
            }
            (IsotropicSpatialStatus::InsufficientPoints, None, None) => {
                if point.directed_pairs != 0
                    || point.visible_arc_evaluations != 0
                    || point.visible_arc_fraction_sum != 0.0
                    || point.inverse_visible_arc_fraction_sum != 0.0
                    || point.lower_l.is_some()
                    || point.upper_l.is_some()
                {
                    return schema("insufficient isotropic curve contains values");
                }
            }
            _ => return schema("isotropic curve availability differs from result status"),
        }
    }
    if previous_radius != geometry.maximum_radius_um
        || analysis.curve.last().is_none_or(|point| {
            point.visible_arc_evaluations != geometry.observed_visible_arc_evaluations
        })
    {
        return schema("isotropic observed radius or arc telemetry differs");
    }
    let config = IsotropicSpatialConfig::new(
        analysis.curve.iter().map(|point| point.radius_um).collect(),
        null.simulations,
        null.seed,
        null.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Schema(error.to_string()))?;
    if configuration.logical_digest != isotropic_configuration_digest(&config).to_string() {
        return schema("isotropic configuration digest differs");
    }
    match analysis.status {
        IsotropicSpatialStatus::Available => {
            let Some(inference) = &analysis.inference else {
                return schema("available isotropic result lacks inference");
            };
            if geometry.point_count < 2
                || inference.simulations != null.simulations
                || inference.eligible_radius_count != analysis.curve.len()
                || !unit_interval(inference.p_global)
                || inference.p_global < 1.0 / (null.simulations + 1) as f64
                || !unit_interval(inference.erl_depth)
                || !unit_interval(inference.critical_depth)
            {
                return schema("isotropic inference summary differs");
            }
        }
        IsotropicSpatialStatus::InsufficientPoints => {
            if geometry.point_count >= 2
                || analysis.inference.is_some()
                || analysis.csr_candidate_draws != 0
                || geometry.total_pair_visits != 0
                || geometry.total_visible_arc_evaluations != 0
                || geometry.total_arc_segment_tests != 0
                || geometry.total_arc_membership_queries != 0
            {
                return schema("insufficient isotropic result is inconsistent");
            }
        }
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn finite_nonnegative(value: Option<f64>) -> bool {
    value.is_some_and(|value| value.is_finite() && value >= 0.0)
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

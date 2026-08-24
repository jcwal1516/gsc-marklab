use serde::{Deserialize, Serialize};

use crate::{
    errors::{MarklabError, Result},
    ClassicalSpatialResult, ClassicalSpatialStatus, KlPointStatus,
};

use super::analysis::classical_configuration_digest;

/// Stable format identifier for standalone classical spatial results.
pub const CLASSICAL_SPATIAL_FORMAT: &str = "marklab.classical_spatial";
/// Current standalone classical spatial result version.
pub const CLASSICAL_SPATIAL_FORMAT_VERSION: &str = "1";

/// Strict standalone document for one classical spatial analysis.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicalSpatialResultDocument {
    format: String,
    format_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workflow: Option<ClassicalWorkflowIdentity>,
    analysis: ClassicalSpatialResult,
}

/// Cache disposition recorded in a standalone project-workflow result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassicalCacheStatus {
    /// The node executed and committed canonical output bytes.
    Miss,
    /// Verified canonical output bytes were replayed.
    Hit,
}

/// Project/cache identity attached after a scheduler execution or replay.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicalWorkflowIdentity {
    /// Deterministic scheduler cache key.
    pub cache_key: String,
    /// Hit or miss disposition.
    pub cache_status: ClassicalCacheStatus,
    /// Canonical private node-output artifact digest.
    pub output_artifact_digest: String,
    /// Exact private node-output artifact byte length.
    pub output_artifact_bytes: u64,
}

impl ClassicalSpatialResultDocument {
    /// Wrap and validate one typed classical analysis result.
    pub fn new(analysis: ClassicalSpatialResult) -> Result<Self> {
        let document = Self {
            format: CLASSICAL_SPATIAL_FORMAT.into(),
            format_version: CLASSICAL_SPATIAL_FORMAT_VERSION.into(),
            workflow: None,
            analysis,
        };
        document.validate()?;
        Ok(document)
    }

    /// Parse and semantically validate one strict version-one document.
    pub fn from_json(text: &str) -> Result<Self> {
        let document: Self = serde_json::from_str(text).map_err(|error| {
            MarklabError::Schema(format!("invalid classical spatial result JSON: {error}"))
        })?;
        document.validate()?;
        Ok(document)
    }

    /// Borrow the typed analysis payload.
    pub fn analysis(&self) -> &ClassicalSpatialResult {
        &self.analysis
    }

    /// Attach exact scheduler/cache identity to a standalone workflow result.
    pub fn with_workflow_identity(mut self, workflow: ClassicalWorkflowIdentity) -> Result<Self> {
        self.workflow = Some(workflow);
        self.validate()?;
        Ok(self)
    }

    /// Borrow scheduler/cache identity when produced through a project workflow.
    pub fn workflow(&self) -> Option<&ClassicalWorkflowIdentity> {
        self.workflow.as_ref()
    }

    /// Consume the document and return the typed analysis payload.
    pub fn into_analysis(self) -> ClassicalSpatialResult {
        self.analysis
    }

    /// Serialize a deterministic, human-readable version-one document.
    pub fn to_json_pretty(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|error| {
            MarklabError::Compute(format!("classical result encoding failed: {error}"))
        })
    }

    fn validate(&self) -> Result<()> {
        if self.format != CLASSICAL_SPATIAL_FORMAT {
            return schema("classical result format is not marklab.classical_spatial");
        }
        if self.format_version != CLASSICAL_SPATIAL_FORMAT_VERSION {
            return Err(MarklabError::UnsupportedFormatVersion {
                found: self.format_version.clone(),
                supported: CLASSICAL_SPATIAL_FORMAT_VERSION.into(),
            });
        }
        if self.workflow.as_ref().is_some_and(|workflow| {
            !valid_digest(&workflow.cache_key)
                || !valid_digest(&workflow.output_artifact_digest)
                || workflow.output_artifact_bytes == 0
        }) {
            return schema("classical workflow identity is invalid");
        }
        validate_analysis(&self.analysis)
    }
}

fn validate_analysis(analysis: &ClassicalSpatialResult) -> Result<()> {
    let window = &analysis.window;
    if window.coordinate_unit != "micrometre"
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
        return schema("classical result has an invalid observation-window summary");
    }
    let geometry = &analysis.geometry;
    if geometry.point_count != analysis.null_design.conditioned_point_count
        || !geometry.maximum_radius_um.is_finite()
        || geometry.maximum_radius_um <= 0.0
        || geometry.total_pair_visits < geometry.observed_pair_visits
        || !valid_digest(&geometry.logical_digest)
        || geometry.execution_mode != "exact"
        || geometry.duplicate_policy != "reject"
        || geometry.boundary_distance_owner != "observation_window_2d"
        || geometry.pair_traversal != "exact_streaming_ordered_pairs"
    {
        return schema("classical result has an invalid geometry summary");
    }
    let configuration = &analysis.configuration;
    if configuration.radius_count != analysis.curve.len()
        || !valid_digest(&configuration.logical_digest)
        || [
            configuration.limits.maximum_points,
            configuration.limits.maximum_radii,
            configuration.limits.maximum_pair_visits,
            configuration.limits.maximum_csr_draws,
            configuration.limits.maximum_retained_bytes,
        ]
        .contains(&0)
        || configuration.limits.maximum_points < geometry.point_count
        || configuration.limits.maximum_radii < analysis.curve.len()
    {
        return schema("classical result has an invalid configuration summary");
    }
    let null = &analysis.null_design;
    if null.simulations == 0
        || !null.alpha.is_finite()
        || null.alpha <= 0.0
        || null.alpha >= 1.0
        || (null.simulations.saturating_add(1) as f64) * null.alpha < 1.0
    {
        return schema("classical result has an invalid null design");
    }
    if analysis.curve.is_empty() {
        return schema("classical result curve must not be empty");
    }

    let mut previous_radius = 0.0;
    let mut inference_eligible_count = 0_usize;
    for point in &analysis.curve {
        if !point.radius_um.is_finite()
            || point.radius_um <= previous_radius
            || !point.theoretical_k.is_finite()
            || point.theoretical_k != std::f64::consts::PI * point.radius_um * point.radius_um
            || point.theoretical_l != point.radius_um
        {
            return schema("classical result curve has invalid radius or theoretical values");
        }
        previous_radius = point.radius_um;
        match point.status {
            KlPointStatus::Available => {
                let (Some(k), Some(l)) = (point.k, point.l) else {
                    return schema("available classical curve points require K and L");
                };
                let expected_denominator = geometry
                    .point_count
                    .checked_mul(point.eligible_centers)
                    .ok_or_else(|| {
                        MarklabError::Schema("classical result count overflow".into())
                    })?;
                let expected_k =
                    window.area_um2 * point.ordered_pairs as f64 / expected_denominator as f64;
                let maximum_pairs = point
                    .eligible_centers
                    .checked_mul(geometry.point_count.saturating_sub(1))
                    .ok_or_else(|| {
                        MarklabError::Schema("classical result count overflow".into())
                    })?;
                if point.eligible_centers == 0
                    || point.eligible_centers > geometry.point_count
                    || point.ordered_pairs > maximum_pairs
                    || !k.is_finite()
                    || k < 0.0
                    || !l.is_finite()
                    || l < 0.0
                    || k != expected_k
                    || l != (k / std::f64::consts::PI).sqrt()
                {
                    return schema(
                        "available classical curve point violates the border K/L identity",
                    );
                }
            }
            KlPointStatus::NoEligibleCenters => {
                if point.eligible_centers != 0
                    || point.ordered_pairs != 0
                    || point.k.is_some()
                    || point.l.is_some()
                {
                    return schema("unavailable classical curve point contains a value");
                }
            }
        }
        if point.inference_eligible {
            inference_eligible_count += 1;
            if point.status != KlPointStatus::Available
                || !finite_nonnegative(point.lower_l)
                || !finite_nonnegative(point.upper_l)
                || point.lower_l > point.upper_l
            {
                return schema("inference-eligible classical curve point has invalid envelopes");
            }
        } else if point.lower_l.is_some() || point.upper_l.is_some() {
            return schema("inference-ineligible classical curve point contains envelopes");
        }
    }
    if geometry.maximum_radius_um != previous_radius {
        return schema("geometry maximum radius does not match the result curve");
    }
    let radii = analysis
        .curve
        .iter()
        .map(|point| point.radius_um)
        .collect::<Vec<_>>();
    let expected_configuration_digest = classical_configuration_digest(
        &radii,
        null.simulations,
        null.seed,
        null.alpha,
        configuration.limits,
    );
    if configuration.logical_digest != expected_configuration_digest.to_string() {
        return schema("classical configuration digest does not match its result fields");
    }

    match analysis.status {
        ClassicalSpatialStatus::Available => {
            let Some(inference) = &analysis.inference else {
                return schema("available classical result requires global inference");
            };
            if inference.simulations != null.simulations
                || inference.eligible_radius_count != inference_eligible_count
                || inference_eligible_count == 0
                || !unit_interval(inference.p_global)
                || inference.p_global < 1.0 / (null.simulations + 1) as f64
                || !unit_interval(inference.erl_depth)
                || !unit_interval(inference.critical_depth)
            {
                return schema("classical global inference summary is inconsistent");
            }
        }
        ClassicalSpatialStatus::InsufficientPoints => {
            if geometry.point_count >= 2
                || analysis.inference.is_some()
                || inference_eligible_count != 0
                || analysis.csr_candidate_draws != 0
            {
                return schema("insufficient-points classical result is inconsistent");
            }
        }
        ClassicalSpatialStatus::InsufficientInferenceSupport => {
            if geometry.point_count < 2
                || analysis.inference.is_some()
                || inference_eligible_count != 0
            {
                return schema("insufficient-inference classical result is inconsistent");
            }
        }
    }
    if geometry.point_count >= 2 {
        let minimum_draws = geometry
            .point_count
            .checked_mul(null.simulations)
            .ok_or_else(|| MarklabError::Schema("classical result count overflow".into()))?;
        if analysis.csr_candidate_draws < minimum_draws {
            return schema("classical result CSR draw count is inconsistent");
        }
    }
    Ok(())
}

fn finite_nonnegative(value: Option<f64>) -> bool {
    value.is_some_and(|value| value.is_finite() && value >= 0.0)
}

fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn schema<T>(message: &str) -> Result<T> {
    Err(MarklabError::Schema(message.into()))
}

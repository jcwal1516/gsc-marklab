use serde::{Deserialize, Serialize};

use crate::{errors::MarklabError, Result};

use super::{analysis::configuration_digest, types::*};

/// Stable standalone nearest/empty-space result format.
pub const NEAREST_SPACE_FORMAT: &str = "marklab.nearest_space";
/// Current standalone nearest/empty-space result version.
pub const NEAREST_SPACE_FORMAT_VERSION: &str = "1";

/// Cache disposition recorded in a project result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NearestSpaceCacheStatus {
    /// The node executed and committed canonical output bytes.
    Miss,
    /// Verified canonical output bytes were replayed.
    Hit,
}

impl NearestSpaceCacheStatus {
    /// Stable lowercase wire value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Miss => "miss",
            Self::Hit => "hit",
        }
    }
}

/// Exact scheduler/cache identity attached to a project result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpaceWorkflowIdentity {
    /// Deterministic scheduler cache key.
    pub cache_key: String,
    /// Hit or miss disposition.
    pub cache_status: NearestSpaceCacheStatus,
    /// Canonical typed node-output digest.
    pub output_artifact_digest: String,
    /// Canonical typed node-output byte length.
    pub output_artifact_bytes: u64,
}

/// Strict standalone document for one F/G/J analysis.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearestSpaceResultDocument {
    format: String,
    format_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workflow: Option<NearestSpaceWorkflowIdentity>,
    analysis: NearestSpaceResult,
}

impl NearestSpaceResultDocument {
    /// Wrap and validate one typed analysis result.
    pub fn new(analysis: NearestSpaceResult) -> Result<Self> {
        let document = Self {
            format: NEAREST_SPACE_FORMAT.into(),
            format_version: NEAREST_SPACE_FORMAT_VERSION.into(),
            workflow: None,
            analysis,
        };
        document.validate()?;
        Ok(document)
    }

    /// Parse and semantically validate one strict version-one document.
    pub fn from_json(text: &str) -> Result<Self> {
        let document: Self = serde_json::from_str(text).map_err(|error| {
            MarklabError::Schema(format!("invalid nearest-space result JSON: {error}"))
        })?;
        document.validate()?;
        Ok(document)
    }

    /// Borrow the typed analysis payload.
    pub fn analysis(&self) -> &NearestSpaceResult {
        &self.analysis
    }

    /// Attach exact scheduler/cache identity.
    pub fn with_workflow_identity(
        mut self,
        workflow: NearestSpaceWorkflowIdentity,
    ) -> Result<Self> {
        self.workflow = Some(workflow);
        self.validate()?;
        Ok(self)
    }

    /// Borrow scheduler/cache identity when present.
    pub fn workflow(&self) -> Option<&NearestSpaceWorkflowIdentity> {
        self.workflow.as_ref()
    }

    /// Consume the document and return its typed payload.
    pub fn into_analysis(self) -> NearestSpaceResult {
        self.analysis
    }

    /// Serialize deterministic human-readable JSON.
    pub fn to_json_pretty(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|error| {
            MarklabError::Compute(format!("nearest-space result encoding failed: {error}"))
        })
    }

    fn validate(&self) -> Result<()> {
        if self.format != NEAREST_SPACE_FORMAT {
            return schema("nearest-space result format is invalid");
        }
        if self.format_version != NEAREST_SPACE_FORMAT_VERSION {
            return Err(MarklabError::UnsupportedFormatVersion {
                found: self.format_version.clone(),
                supported: NEAREST_SPACE_FORMAT_VERSION.into(),
            });
        }
        if self.workflow.as_ref().is_some_and(|identity| {
            !valid_digest(&identity.cache_key)
                || !valid_digest(&identity.output_artifact_digest)
                || identity.output_artifact_bytes == 0
        }) {
            return schema("nearest-space workflow identity is invalid");
        }
        validate_analysis(&self.analysis)
    }
}

fn validate_analysis(result: &NearestSpaceResult) -> Result<()> {
    if result.curve.is_empty()
        || result.configuration.radius_count != result.curve.len()
        || !valid_digest(&result.configuration.logical_digest)
        || !valid_digest(&result.geometry.logical_digest)
        || !valid_digest(&result.probes.logical_digest)
        || result.geometry.point_count != result.null_design.conditioned_point_count
        || result.geometry.nearest_query_count > result.configuration.limits.maximum_nearest_queries
        || result.geometry.estimated_storage_bytes
            > result.configuration.limits.maximum_retained_bytes
        || result.geometry.nearest_neighbor_owner != "spatial_index_2d"
        || result.geometry.edge_correction != "standard_border_reduced_sample"
        || result.probes.method != "fixed_cell_centred_rectangular_grid"
        || result.probes.requested_grid.contains(&0)
        || result.probes.retained_probe_count == 0
        || result.probes.retained_probe_count > result.configuration.limits.maximum_probes
        || result
            .probes
            .spacing_um
            .iter()
            .any(|value| !finite_positive(*value))
        || !finite_positive(result.probes.maximum_location_error_um)
    {
        return schema("nearest-space result has invalid identity or resource metadata");
    }
    let radii = result
        .curve
        .iter()
        .map(|point| point.radius_um)
        .collect::<Vec<_>>();
    let config = NearestSpaceConfig::new(
        radii,
        result.probes.requested_grid,
        result.null_design.simulations,
        result.null_design.seed,
        result.null_design.alpha,
        result.configuration.j_denominator_epsilon,
        result.configuration.limits,
    )
    .map_err(|error| MarklabError::Schema(error.to_string()))?;
    if configuration_digest(&config).to_string() != result.configuration.logical_digest
        || result.geometry.maximum_radius_um != *config.radii_um().last().expect("radii")
    {
        return schema("nearest-space configuration digest is inconsistent");
    }
    let mut f_eligible = 0_usize;
    let mut g_eligible = 0_usize;
    let mut j_eligible = 0_usize;
    for point in &result.curve {
        validate_distribution(
            point.f_status,
            point.eligible_probes,
            point.probes_with_event_within_radius,
            point.f,
        )?;
        validate_distribution(
            point.g_status,
            point.eligible_event_centers,
            point.events_with_neighbor_within_radius,
            point.g,
        )?;
        match point.j_status {
            JPointStatus::Available => {
                let (Some(f), Some(g), Some(j)) = (point.f, point.g, point.j) else {
                    return schema("available J requires finite F, G, and J");
                };
                if 1.0 - f <= result.configuration.j_denominator_epsilon
                    || !same_calculated_float(j, (1.0 - g) / (1.0 - f))
                {
                    return schema("nearest-space J identity is invalid");
                }
            }
            JPointStatus::DenominatorTooSmall => {
                if point.f.is_none()
                    || point.g.is_none()
                    || point.j.is_some()
                    || 1.0 - point.f.expect("checked F")
                        > result.configuration.j_denominator_epsilon
                {
                    return schema("nearest-space J denominator status is inconsistent");
                }
            }
            JPointStatus::MissingComponent => {
                if point.j.is_some() || (point.f.is_some() && point.g.is_some()) {
                    return schema("nearest-space missing-component J is inconsistent");
                }
            }
        }
        f_eligible += validate_envelope(
            point.f_inference_eligible,
            point.lower_f,
            point.upper_f,
            point.f,
        )?;
        g_eligible += validate_envelope(
            point.g_inference_eligible,
            point.lower_g,
            point.upper_g,
            point.g,
        )?;
        j_eligible += validate_envelope(
            point.j_inference_eligible,
            point.lower_j,
            point.upper_j,
            point.j,
        )?;
    }
    validate_inference(result.inference.f.as_ref(), f_eligible, result)?;
    validate_inference(result.inference.g.as_ref(), g_eligible, result)?;
    validate_inference(result.inference.j.as_ref(), j_eligible, result)?;
    match result.status {
        NearestSpaceStatus::Available => {
            if result.geometry.point_count < 2
                || (result.inference.f.is_none()
                    && result.inference.g.is_none()
                    && result.inference.j.is_none())
            {
                return schema("available nearest-space result lacks inference support");
            }
        }
        NearestSpaceStatus::InsufficientEvents => {
            if result.geometry.point_count >= 2 {
                return schema("insufficient-events nearest-space status is inconsistent");
            }
        }
        NearestSpaceStatus::InsufficientInferenceSupport => {
            if result.geometry.point_count < 2
                || result.inference.f.is_some()
                || result.inference.g.is_some()
                || result.inference.j.is_some()
            {
                return schema("nearest-space inference-support status is inconsistent");
            }
        }
    }
    Ok(())
}

fn validate_distribution(
    status: DistributionPointStatus,
    denominator: usize,
    numerator: usize,
    value: Option<f64>,
) -> Result<()> {
    if numerator > denominator {
        return schema("nearest-space distribution count is invalid");
    }
    match status {
        DistributionPointStatus::Available => {
            let Some(value) = value else {
                return schema("available nearest-space distribution lacks a value");
            };
            if denominator == 0
                || !unit_interval(value)
                || !same_calculated_float(value, numerator as f64 / denominator as f64)
            {
                return schema("nearest-space distribution identity is invalid");
            }
        }
        DistributionPointStatus::NoEligibleCenters => {
            if denominator != 0 || numerator != 0 || value.is_some() {
                return schema("no-eligible-centers distribution is inconsistent");
            }
        }
        DistributionPointStatus::InsufficientEvents => {
            if value.is_some() || numerator != 0 {
                return schema("insufficient-events G distribution is inconsistent");
            }
        }
    }
    Ok(())
}

fn validate_envelope(
    eligible: bool,
    lower: Option<f64>,
    upper: Option<f64>,
    observed: Option<f64>,
) -> Result<usize> {
    if eligible {
        if observed.is_none()
            || !lower.is_some_and(f64::is_finite)
            || !upper.is_some_and(f64::is_finite)
            || lower > upper
        {
            return schema("nearest-space eligible envelope is invalid");
        }
        Ok(1)
    } else if lower.is_some() || upper.is_some() {
        schema("nearest-space ineligible radius contains an envelope")
    } else {
        Ok(0)
    }
}

fn validate_inference(
    inference: Option<&NearestSpaceComponentInference>,
    eligible_count: usize,
    result: &NearestSpaceResult,
) -> Result<()> {
    match inference {
        Some(inference)
            if inference.eligible_radius_count == eligible_count
                && eligible_count > 0
                && unit_interval(inference.p_global)
                && inference.p_global >= 1.0 / (result.null_design.simulations + 1) as f64
                && unit_interval(inference.erl_depth)
                && unit_interval(inference.critical_depth) =>
        {
            Ok(())
        }
        None if eligible_count == 0 => Ok(()),
        _ => schema("nearest-space component inference is inconsistent"),
    }
}

fn finite_positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn same_calculated_float(actual: f64, expected: f64) -> bool {
    actual == expected
        || (actual.is_finite()
            && expected.is_finite()
            && (actual - expected).abs()
                <= 16.0 * f64::EPSILON * actual.abs().max(expected.abs()).max(1.0))
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

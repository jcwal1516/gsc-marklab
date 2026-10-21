use crate::edge_corrected::document::{self, valid_digest, Analysis, Point, Statistic};
use crate::result_vocabulary::{
    ArcMeasureOwner, DuplicatePointPolicy, EdgeCorrection, PairTraversal, SpatialExecutionMode,
};
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
    let geometry = &analysis.geometry;
    let configuration = &analysis.configuration;
    let limits = configuration.limits;
    let null = &analysis.null_design;
    if analysis.correction != EdgeCorrection::Isotropic
        || !valid_digest(&geometry.logical_digest)
        || geometry.execution_mode != SpatialExecutionMode::Exact
        || geometry.duplicate_policy != DuplicatePointPolicy::Reject
        || geometry.visible_arc_owner != ArcMeasureOwner::ObservationWindow2dSegmentCirclePartition
        || geometry.pair_traversal
            != PairTraversal::ExactStreamingUnorderedPairsWithTwoDirectedArcWeights
        || configuration.radius_count != analysis.curve.len()
        || !valid_digest(&configuration.logical_digest)
    {
        return schema("isotropic estimator identity or configuration is invalid");
    }
    document::validate_analysis(
        Analysis {
            isotropic: true,
            statistic: Statistic::Kl {
                available: analysis.status == IsotropicSpatialStatus::Available,
            },
            window: &analysis.window,
            null,
            inference: analysis.inference.as_ref(),
            point_count: geometry.point_count,
            maximum_radius: geometry.maximum_radius_um,
            observed_pairs: geometry.observed_pair_visits,
            total_pairs: geometry.total_pair_visits,
            observed_evaluations: geometry.observed_visible_arc_evaluations,
            total_evaluations: geometry.total_visible_arc_evaluations,
            total_work: geometry.total_arc_segment_tests,
            auxiliary_work: geometry.total_arc_membership_queries,
            maximum_angles: geometry.maximum_boundary_intersection_angles,
            csr_draws: analysis.csr_candidate_draws,
            curve_count: analysis.curve.len(),
            resources: [
                (geometry.point_count, limits.maximum_points),
                (analysis.curve.len(), limits.maximum_radii),
                (geometry.total_pair_visits, limits.maximum_pair_visits),
                (
                    geometry.total_visible_arc_evaluations,
                    limits.maximum_visible_arc_evaluations,
                ),
                (
                    geometry.total_arc_segment_tests,
                    limits.maximum_arc_segment_tests,
                ),
                (
                    geometry.total_arc_membership_queries,
                    limits.maximum_arc_membership_queries,
                ),
                (analysis.csr_candidate_draws, limits.maximum_csr_draws),
                (
                    geometry.estimated_storage_bytes,
                    limits.maximum_retained_bytes,
                ),
            ],
        },
        analysis.curve.iter().map(|point| Point {
            radius: point.radius_um,
            pairs: point.directed_pairs,
            evaluations: point.visible_arc_evaluations,
            measure: point.visible_arc_fraction_sum,
            weight: point.inverse_visible_arc_fraction_sum,
            value: point.k,
            l: point.l,
            theoretical: point.theoretical_k,
            theoretical_l: point.theoretical_l,
            lower: point.lower_l,
            upper: point.upper_l,
            status: None,
            eligible: false,
        }),
    )?;
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
    Ok(())
}

fn schema<T>(reason: &str) -> Result<T> {
    Err(MarklabError::Schema(reason.into()))
}

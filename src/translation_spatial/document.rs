use crate::edge_corrected::document::{self, valid_digest, Analysis, Point, Statistic};
use crate::result_vocabulary::{
    DuplicatePointPolicy, EdgeCorrection, OverlapMeasureOwner, PairTraversal, SpatialExecutionMode,
};
use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};

use super::{
    analysis::translation_configuration_digest, TranslationSpatialConfig, TranslationSpatialResult,
    TranslationSpatialStatus,
};

/// Stable format identifier for translation-corrected spatial results.
pub const TRANSLATION_SPATIAL_FORMAT: &str = "marklab.translation_spatial";
/// Current standalone translation result version.
pub const TRANSLATION_SPATIAL_FORMAT_VERSION: &str = "1";

/// Strict standalone document for one translation-corrected K/L analysis.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationSpatialResultDocument {
    format: String,
    format_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workflow: Option<TranslationWorkflowIdentity>,
    analysis: TranslationSpatialResult,
}

/// Durable cache disposition attached by the project CLI.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationCacheStatus {
    /// The node executed and committed canonical output.
    Miss,
    /// Verified canonical output was replayed.
    Hit,
}

/// Exact scheduler/cache identity attached to a standalone result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationWorkflowIdentity {
    /// Scheduler cache key.
    pub cache_key: String,
    /// Hit or miss disposition.
    pub cache_status: TranslationCacheStatus,
    /// Canonical node-output artifact digest.
    pub output_artifact_digest: String,
    /// Exact node-output bytes.
    pub output_artifact_bytes: u64,
}

impl TranslationSpatialResultDocument {
    /// Wrap and validate a typed translation result.
    pub fn new(analysis: TranslationSpatialResult) -> Result<Self> {
        let document = Self {
            format: TRANSLATION_SPATIAL_FORMAT.into(),
            format_version: TRANSLATION_SPATIAL_FORMAT_VERSION.into(),
            workflow: None,
            analysis,
        };
        document.validate()?;
        Ok(document)
    }

    /// Decode and semantically validate strict version-one JSON.
    pub fn from_json(text: &str) -> Result<Self> {
        let document: Self = serde_json::from_str(text).map_err(|error| {
            MarklabError::Schema(format!("invalid translation spatial result JSON: {error}"))
        })?;
        document.validate()?;
        Ok(document)
    }

    /// Borrow the typed analysis.
    pub fn analysis(&self) -> &TranslationSpatialResult {
        &self.analysis
    }

    /// Consume the wrapper and return the analysis.
    pub fn into_analysis(self) -> TranslationSpatialResult {
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
        serde_json::to_string_pretty(self).map_err(|error| {
            MarklabError::Compute(format!("translation result encoding failed: {error}"))
        })
    }

    fn validate(&self) -> Result<()> {
        if self.format != TRANSLATION_SPATIAL_FORMAT {
            return schema("translation result format differs");
        }
        if self.format_version != TRANSLATION_SPATIAL_FORMAT_VERSION {
            return Err(MarklabError::UnsupportedFormatVersion {
                found: self.format_version.clone(),
                supported: TRANSLATION_SPATIAL_FORMAT_VERSION.into(),
            });
        }
        if self.workflow.as_ref().is_some_and(|workflow| {
            !valid_digest(&workflow.cache_key)
                || !valid_digest(&workflow.output_artifact_digest)
                || workflow.output_artifact_bytes == 0
        }) {
            return schema("translation workflow identity is invalid");
        }
        validate_analysis(&self.analysis)
    }
}

fn validate_analysis(analysis: &TranslationSpatialResult) -> Result<()> {
    let geometry = &analysis.geometry;
    let configuration = &analysis.configuration;
    let limits = configuration.limits;
    let null = &analysis.null_design;
    if analysis.correction != EdgeCorrection::Translation
        || !valid_digest(&geometry.logical_digest)
        || geometry.execution_mode != SpatialExecutionMode::Exact
        || geometry.duplicate_policy != DuplicatePointPolicy::Reject
        || geometry.overlap_owner
            != OverlapMeasureOwner::ObservationWindow2dExactBooleanIntersection
        || geometry.pair_traversal
            != PairTraversal::ExactStreamingUnorderedPairsWithBothOrderedWeights
        || configuration.radius_count != analysis.curve.len()
        || !valid_digest(&configuration.logical_digest)
    {
        return schema("translation estimator identity or configuration is invalid");
    }
    document::validate_analysis(
        Analysis {
            isotropic: false,
            statistic: Statistic::Kl {
                available: analysis.status == TranslationSpatialStatus::Available,
            },
            window: &analysis.window,
            null,
            inference: analysis.inference.as_ref(),
            point_count: geometry.point_count,
            maximum_radius: geometry.maximum_radius_um,
            observed_pairs: geometry.observed_pair_visits,
            total_pairs: geometry.total_pair_visits,
            observed_evaluations: geometry.observed_overlap_evaluations,
            total_evaluations: geometry.total_overlap_evaluations,
            total_work: geometry.total_overlap_candidate_work,
            auxiliary_work: geometry.maximum_overlap_output_vertices_observed,
            maximum_angles: 0,
            csr_draws: analysis.csr_candidate_draws,
            curve_count: analysis.curve.len(),
            resources: [
                (geometry.point_count, limits.maximum_points),
                (analysis.curve.len(), limits.maximum_radii),
                (geometry.total_pair_visits, limits.maximum_pair_visits),
                (
                    geometry.total_overlap_evaluations,
                    limits.maximum_overlap_evaluations,
                ),
                (
                    geometry.total_overlap_candidate_work,
                    limits.maximum_overlap_candidate_work,
                ),
                (
                    geometry.maximum_overlap_output_vertices_observed,
                    limits.maximum_overlap_output_vertices,
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
            pairs: point.ordered_pairs,
            evaluations: point.overlap_evaluations,
            measure: point.translation_overlap_area_sum_um2,
            weight: point.translation_weight_sum,
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
    let config = TranslationSpatialConfig::new(
        analysis.curve.iter().map(|point| point.radius_um).collect(),
        null.simulations,
        null.seed,
        null.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Schema(error.to_string()))?;
    if configuration.logical_digest != translation_configuration_digest(&config).to_string() {
        return schema("translation configuration digest differs");
    }
    Ok(())
}

fn schema<T>(reason: &str) -> Result<T> {
    Err(MarklabError::Schema(reason.into()))
}

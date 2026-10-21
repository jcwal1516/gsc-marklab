use crate::edge_corrected::document::{self, valid_digest, Analysis, Point, Statistic};
use crate::result_vocabulary::{
    DuplicatePointPolicy, EdgeCorrection, OverlapMeasureOwner, PairTraversal, SpatialExecutionMode,
};
use serde::{Deserialize, Serialize};

use crate::{
    errors::{MarklabError, Result},
    spatial::{
        PairCorrelationKernel, TranslationPairCorrelationConfig, TranslationPairCorrelationResult,
        TranslationWorkflowIdentity,
    },
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
        serde_json::to_string_pretty(self).map_err(|error| {
            MarklabError::Compute(format!(
                "translation pair-correlation result encoding failed: {error}"
            ))
        })
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

fn validate_analysis(analysis: &TranslationPairCorrelationResult) -> Result<()> {
    let geometry = &analysis.geometry;
    let configuration = &analysis.configuration;
    let limits = configuration.limits;
    let null = &analysis.null_design;
    if analysis.correction != EdgeCorrection::Translation
        || analysis.kernel != PairCorrelationKernel::Epanechnikov
        || !analysis.bandwidth_um.is_finite()
        || analysis.bandwidth_um <= 0.0
        || analysis.randomization_unit
            != crate::spatial::ClassicalRandomizationUnit::WholeLocationPattern
        || !valid_digest(&geometry.logical_digest)
        || geometry.execution_mode != SpatialExecutionMode::Exact
        || geometry.duplicate_policy != DuplicatePointPolicy::Reject
        || geometry.overlap_owner
            != OverlapMeasureOwner::ObservationWindow2dExactBooleanIntersection
        || geometry.pair_traversal
            != PairTraversal::ExactStreamingUnorderedCompactSupportPairsWithBothOrderedWeights
        || configuration.radius_count != analysis.curve.len()
        || !valid_digest(&configuration.logical_digest)
    {
        return schema("translation estimator identity or configuration is invalid");
    }
    document::validate_analysis(
        Analysis {
            isotropic: false,
            statistic: Statistic::PairCorrelation {
                bandwidth: analysis.bandwidth_um,
            },
            window: &analysis.window,
            null,
            inference: analysis.inference.as_ref(),
            point_count: geometry.point_count,
            maximum_radius: geometry.maximum_query_radius_um,
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
            pairs: point.ordered_pairs_in_support,
            evaluations: point.overlap_evaluations,
            measure: point.translation_overlap_area_sum_um2,
            weight: point.translation_weighted_kernel_sum,
            value: point.g,
            l: None,
            theoretical: point.theoretical_g,
            theoretical_l: 0.0,
            lower: point.lower_g,
            upper: point.upper_g,
            status: Some(point.status),
            eligible: point.inference_eligible,
        }),
    )?;
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
        return schema("translation configuration digest differs");
    }
    Ok(())
}

fn schema<T>(reason: &str) -> Result<T> {
    Err(MarklabError::Schema(reason.into()))
}

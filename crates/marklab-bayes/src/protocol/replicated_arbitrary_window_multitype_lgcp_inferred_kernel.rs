//! Replicated multitype LGCP whose shared Matern-3/2 kernel scales are inferred in the model.
//!
//! Likelihood, quadrature, fixed effects, hierarchy priors and centring are those of
//! `replicated_arbitrary_window_multitype_lgcp`. One amplitude and one length scale are shared
//! by all types and patterns: `field_amplitude ~ HalfNormal(field_amplitude_scale)` and
//! `field_length_scale_um ~ HalfNormal(field_length_scale_scale_um)`. In each pattern every type
//! has an independent field `chol(K + jitter I) z`, centred within the pattern, with
//! `K = field_amplitude^2 (1 + sqrt(3) d / l) exp(-sqrt(3) d / l)`; no cross-type covariance is
//! modelled. The embedded fixed-kernel source request must carry the same prior scales and
//! jitter. A complete fit (finiteness, centring and the NUTS policy) claims
//! `experimental_inferred_shared_multitype_kernel`, otherwise `diagnostic_only_nonconverged`.

use super::posterior_validation::{diagnostics_pass, summary_valid};
use super::replicated_arbitrary_window_multitype_lgcp::hierarchy_effects_valid;
use super::replicated_arbitrary_window_multitype_lgcp::pair_differences_valid;
use super::replicated_arbitrary_window_multitype_lgcp::type_posteriors_valid;
use super::replicated_arbitrary_window_multitype_lgcp::HierarchyEffect;
use super::replicated_arbitrary_window_multitype_lgcp::NodeTypePosterior;
use super::replicated_arbitrary_window_multitype_lgcp::PairDifference;
use super::replicated_arbitrary_window_multitype_lgcp::PatternTypePredictive;
use super::replicated_arbitrary_window_multitype_lgcp::TypePosterior;
use crate::BackendContract;
use crate::BayesError;
use crate::DiagnosticPolicy;
use crate::FitState;
use crate::McseScalarSummary;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

pub struct ValidationContext<'a> {
    pub source_value: &'a serde_json::Value,
    pub backend: &'a BackendContract,
    pub source_backend: &'a BackendContract,
    pub request_sha256: &'a str,
    pub source_request_sha256: &'a str,
    pub resources: &'a KernelResources,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelPriors {
    pub field_amplitude_scale: f64,
    pub field_length_scale_scale_um: f64,
    pub jitter: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelResources {
    pub maximum_kernel_cube_work: u64,
    pub kernel_cube_work: u64,
    pub maximum_tree_depth: u32,
    pub timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelPosterior {
    pub field_amplitude: McseScalarSummary,
    pub field_length_scale_um: McseScalarSummary,
}

#[derive(Serialize)]
pub struct WorkerRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub source_backend: BackendContract,
    pub source_request_sha256: &'a str,
    pub source_request: serde_json::Value,
    pub kernel_priors: KernelPriors,
    pub resources: KernelResources,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub source_backend: WorkerBackend,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub kernel_posterior: KernelPosterior,
    pub type_posteriors: Vec<TypePosterior>,
    pub group_effect_differences: Vec<PairDifference>,
    pub patient_type_effects: Vec<HierarchyEffect>,
    pub pattern_type_effects: Vec<HierarchyEffect>,
    pub node_type_posteriors: Vec<NodeTypePosterior>,
    pub pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
    pub diagnostics: PosteriorDiagnostics,
    pub kernel_cube_work: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub source_backend: WorkerBackend,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub kernel_posterior: KernelPosterior,
    pub type_posteriors: Vec<TypePosterior>,
    pub group_effect_differences: Vec<PairDifference>,
    pub patient_type_effects: Vec<HierarchyEffect>,
    pub pattern_type_effects: Vec<HierarchyEffect>,
    pub node_type_posteriors: Vec<NodeTypePosterior>,
    pub pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
    pub diagnostics: PosteriorDiagnostics,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub type_count: usize,
    pub total_node_count: usize,
    pub total_node_type_row_count: usize,
    pub total_event_count: u64,
    pub groups: [String; 2],
    pub reference_type: String,
    pub cohort_count: usize,
    pub kernel_priors: KernelPriors,
    pub resources: KernelResources,
    pub statistical_unit: StatisticalUnit,
    pub pattern_unit: String,
    pub likelihood: String,
    pub null_model: NullHypothesis,
    pub assumptions: Vec<String>,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

impl Output {
    pub fn validate_for(&self, expected: &ValidationContext<'_>) -> Result<(), BayesError> {
        let mut value = serde_json::to_value(self)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| BayesError::WorkerContract("inferred-kernel result differs".into()))?;
        for name in [
            "patient_count",
            "pattern_count",
            "type_count",
            "total_node_count",
            "total_node_type_row_count",
            "total_event_count",
            "groups",
            "reference_type",
            "cohort_count",
            "kernel_priors",
            "resources",
            "statistical_unit",
            "pattern_unit",
            "likelihood",
            "null_model",
            "assumptions",
            "finite_result_policy",
            "claim_status",
        ] {
            object.remove(name);
        }
        object.insert(
            "kernel_cube_work".into(),
            expected.resources.kernel_cube_work.into(),
        );
        let result: WorkerResult = serde_json::from_value(value)?;
        validate_result(&result, expected)?;
        let source = &expected.source_value;
        if self.patient_count != array(source, "patients")?.len()
            || self.pattern_count != array(source, "patterns")?.len()
            || self.type_count != array(source, "type_ids")?.len()
            || self.total_node_count != array(source, "nodes")?.len()
            || self.total_node_type_row_count != array(source, "node_type_counts")?.len()
            || self.statistical_unit != StatisticalUnit::Patient
            || self.pattern_unit != "slide_pattern_nested_in_patient"
        {
            return Err(BayesError::WorkerContract(
                "durable replicated multitype inferred-kernel metadata differs".into(),
            ));
        }
        Ok(())
    }
}

pub fn validate_result(
    result: &WorkerResult,
    expected: &ValidationContext<'_>,
) -> Result<(), BayesError> {
    let source = &expected.source_value;
    let types = strings(source, "type_ids")?;
    let patients = identities(array(source, "patients")?, "patient_id")?;
    let patterns = identities(array(source, "patterns")?, "pattern_id")?;
    let nodes = array(source, "nodes")?;
    let counts = array(source, "node_type_counts")?;
    let policy = DiagnosticPolicy::default();
    let diagnostics_pass = diagnostics_pass(&result.diagnostics, &policy);
    let nodes_valid = result.node_type_posteriors.len() == counts.len()
        && result
            .node_type_posteriors
            .iter()
            .zip(counts)
            .all(|(row, count)| {
                let Some(node_index) = count["node_index"].as_u64().map(|value| value as usize)
                else {
                    return false;
                };
                let Some(type_index) = count["type_index"].as_u64().map(|value| value as usize)
                else {
                    return false;
                };
                nodes.get(node_index).is_some_and(|node| {
                    row.pattern_id == node["pattern_id"]
                        && row.node_id == node["node_id"]
                        && types.get(type_index) == Some(&row.type_id)
                        && summary_valid(&row.latent_effect)
                        && summary_valid(&row.expected_count)
                        && row.expected_count.mean > 0.0
                })
            });
    let complete_draws = source["sampling"]["chains"].as_u64().unwrap_or(0)
        * source["sampling"]["draws_per_chain"].as_u64().unwrap_or(0);
    if result.format
        != "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_fit"
        || result.version != 1
        || !result.backend.matches(expected.backend)
        || !result.source_backend.matches(expected.source_backend)
        || result.input_sha256 != source["input_sha256"]
        || result.request_sha256 != expected.request_sha256
        || result.source_request_sha256 != expected.source_request_sha256
        || result.sampling.completed_draws != complete_draws
        || result.kernel_cube_work != expected.resources.kernel_cube_work
        || !summary_valid(&result.kernel_posterior.field_amplitude)
        || !summary_valid(&result.kernel_posterior.field_length_scale_um)
        || result.kernel_posterior.field_amplitude.mean <= 0.0
        || result.kernel_posterior.field_length_scale_um.mean <= 0.0
        || !type_posteriors_valid(&result.type_posteriors, &types)
        || !pair_differences_valid(&result.group_effect_differences, &types)
        || !hierarchy_effects_valid(
            &result.patient_type_effects,
            patients.iter().map(String::as_str),
            &types,
        )
        || !hierarchy_effects_valid(
            &result.pattern_type_effects,
            patterns.iter().map(String::as_str),
            &types,
        )
        || !nodes_valid
        || !predictive_valid(
            &result.pattern_type_posterior_predictive,
            source,
            complete_draws,
        )
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesError::WorkerContract(
            "replicated multitype inferred-kernel result differs".into(),
        ));
    }
    Ok(())
}

pub fn predictive_valid(
    rows: &[PatternTypePredictive],
    source: &serde_json::Value,
    draws: u64,
) -> bool {
    let Ok(patterns) = array(source, "patterns") else {
        return false;
    };
    let Ok(types) = strings(source, "type_ids") else {
        return false;
    };
    let Ok(nodes) = array(source, "nodes") else {
        return false;
    };
    let Ok(counts) = array(source, "node_type_counts") else {
        return false;
    };
    rows.len() == patterns.len() * types.len()
        && rows.iter().enumerate().all(|(index, row)| {
            let pattern_index = index / types.len();
            let type_index = index % types.len();
            let observed = counts
                .iter()
                .filter(|count| {
                    count["type_index"].as_u64() == Some(type_index as u64)
                        && count["node_index"].as_u64().is_some_and(|node| {
                            nodes[node as usize]["pattern_index"].as_u64()
                                == Some(pattern_index as u64)
                        })
                })
                .filter_map(|count| count["count"].as_u64())
                .sum::<u64>();
            let finite = [
                row.replicated_total_count_mean,
                row.replicated_total_count_sd,
                row.replicated_total_count_interval_lower,
                row.replicated_total_count_interval_upper,
                row.total_count_two_sided_tail_probability,
                row.observed_node_count_variance,
                row.replicated_node_count_variance_mean,
                row.node_variance_two_sided_tail_probability,
            ]
            .into_iter()
            .all(f64::is_finite);
            row.pattern_id == patterns[pattern_index]["pattern_id"]
                && row.type_id == types[type_index]
                && row.observed_total_count == observed
                && row.replicate_count == draws
                && finite
                && row.replicated_total_count_sd >= 0.0
                && row.replicated_total_count_interval_lower
                    <= row.replicated_total_count_interval_upper
                && row.observed_node_count_variance >= 0.0
                && row.replicated_node_count_variance_mean >= 0.0
                && (0.0..=1.0).contains(&row.total_count_two_sided_tail_probability)
                && (0.0..=1.0).contains(&row.node_variance_two_sided_tail_probability)
        })
}

pub fn array<'a>(
    value: &'a serde_json::Value,
    name: &str,
) -> Result<&'a [serde_json::Value], BayesError> {
    value[name]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| BayesError::InvalidSpec(format!("prepared {name} differ")))
}

pub fn strings(value: &serde_json::Value, name: &str) -> Result<Vec<String>, BayesError> {
    array(value, name)?
        .iter()
        .map(|row| {
            row.as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| BayesError::InvalidSpec(format!("prepared {name} differ")))
        })
        .collect()
}

fn identities(rows: &[serde_json::Value], name: &str) -> Result<Vec<String>, BayesError> {
    rows.iter()
        .map(|row| {
            row[name]
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| BayesError::InvalidSpec(format!("prepared {name} differs")))
        })
        .collect()
}

pub fn string(value: &serde_json::Value, name: &str) -> Result<String, BayesError> {
    value[name]
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| BayesError::InvalidSpec(format!("prepared {name} differs")))
}

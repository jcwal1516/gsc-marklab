//! Typed messages and scientific checks for replicated arbitrary window multitype lgcp sensitivity.

use super::posterior_validation::diagnostics_pass;
use super::posterior_validation::summary_valid;
use crate::backend_versions::JAX_VERSION;
use crate::BackendContract;
use crate::BayesError;
use crate::DiagnosticPolicy;
use crate::FitState;
use crate::McseScalarSummary;
use crate::PosteriorDiagnostics;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
pub struct TypeRow {
    pub type_id: String,
    pub intercept: McseScalarSummary,
    pub group_effect: McseScalarSummary,
    pub covariate_effect: McseScalarSummary,
    pub patient_sd: McseScalarSummary,
    pub pattern_sd: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct DifferenceRow {
    pub type_a: String,
    pub type_b: String,
    pub difference: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct EffectRow {
    pub owner_id: String,
    pub type_id: String,
    pub effect: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct NodeRow {
    pub pattern_id: String,
    pub node_id: String,
    pub type_id: String,
    pub latent_effect: McseScalarSummary,
    pub expected_count: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct BaselineView {
    pub input_sha256: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub type_posteriors: Vec<TypeRow>,
    pub group_effect_differences: Vec<DifferenceRow>,
    pub patient_type_effects: Vec<EffectRow>,
    pub pattern_type_effects: Vec<EffectRow>,
    pub node_type_posteriors: Vec<NodeRow>,
}

#[derive(Debug, Deserialize)]
pub struct ScenarioView {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub maximum_tree_depth: u32,
    pub fit_state: FitState,
    pub type_posteriors: Vec<TypeRow>,
    pub group_effect_differences: Vec<DifferenceRow>,
    pub patient_type_effects: Vec<EffectRow>,
    pub pattern_type_effects: Vec<EffectRow>,
    pub node_type_posteriors: Vec<NodeRow>,
    pub diagnostics: PosteriorDiagnostics,
}

#[derive(Debug, Serialize)]
pub struct ScenarioOutput {
    pub scenario: &'static str,
    pub patient_sd_prior_scale: f64,
    pub pattern_sd_prior_scale: f64,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub maximum_type_group_shift_sd: f64,
    pub maximum_pairwise_group_difference_shift_sd: f64,
    pub maximum_patient_effect_shift_sd: f64,
    pub maximum_pattern_effect_shift_sd: f64,
    pub maximum_hierarchy_scale_shift_sd: f64,
    pub maximum_expected_count_shift_sd: f64,
    pub material_sensitivity: bool,
    pub fit: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct Resources {
    pub scenario_count: usize,
    pub maximum_processes: usize,
    pub per_scenario_draw_node_type_work: u64,
    pub total_draw_node_type_work: u64,
    pub maximum_total_draw_node_type_work: u64,
    pub timeout_seconds_per_process: u64,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub format: &'static str,
    pub version: u32,
    pub baseline_request_sha256: String,
    pub scenarios: Vec<ScenarioOutput>,
    pub materiality_standard_deviations: f64,
    pub material_sensitivity: bool,
    pub fit_state: FitState,
    pub resources: Resources,
    pub statistical_unit: StatisticalUnit,
    pub assumptions: [&'static str; 5],
    pub finite_result_policy: &'static str,
    pub claim_status: ClaimStatus,
}

pub fn validate_scenario(
    result: &ScenarioView,
    baseline: &BaselineView,
    source_request_sha256: &str,
    request_sha: &str,
    backend: &BackendContract,
    depth: u32,
) -> Result<(), BayesError> {
    let policy = DiagnosticPolicy::default();
    let complete = diagnostics_pass(&result.diagnostics, &policy);
    let identities = row_identities_match(baseline, result);
    if result.format != "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != baseline.input_sha256
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_request_sha256
        || result.maximum_tree_depth != depth
        || !identities
        || !scenario_summaries_valid(result)
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesError::WorkerContract(
            "replicated multitype LGCP sensitivity result differs".into(),
        ));
    }
    Ok(())
}

fn scenario_summaries_valid(result: &ScenarioView) -> bool {
    result
        .type_posteriors
        .iter()
        .flat_map(|row| {
            [
                &row.intercept,
                &row.group_effect,
                &row.covariate_effect,
                &row.patient_sd,
                &row.pattern_sd,
            ]
        })
        .chain(
            result
                .group_effect_differences
                .iter()
                .map(|row| &row.difference),
        )
        .chain(result.patient_type_effects.iter().map(|row| &row.effect))
        .chain(result.pattern_type_effects.iter().map(|row| &row.effect))
        .chain(
            result
                .node_type_posteriors
                .iter()
                .flat_map(|row| [&row.latent_effect, &row.expected_count]),
        )
        .all(summary_valid)
}

fn row_identities_match(left: &BaselineView, right: &ScenarioView) -> bool {
    left.type_posteriors.len() == right.type_posteriors.len()
        && left
            .type_posteriors
            .iter()
            .zip(&right.type_posteriors)
            .all(|(a, b)| a.type_id == b.type_id)
        && left.group_effect_differences.len() == right.group_effect_differences.len()
        && left
            .group_effect_differences
            .iter()
            .zip(&right.group_effect_differences)
            .all(|(a, b)| a.type_a == b.type_a && a.type_b == b.type_b)
        && left.patient_type_effects.len() == right.patient_type_effects.len()
        && left
            .patient_type_effects
            .iter()
            .zip(&right.patient_type_effects)
            .all(|(a, b)| a.owner_id == b.owner_id && a.type_id == b.type_id)
        && left.pattern_type_effects.len() == right.pattern_type_effects.len()
        && left
            .pattern_type_effects
            .iter()
            .zip(&right.pattern_type_effects)
            .all(|(a, b)| a.owner_id == b.owner_id && a.type_id == b.type_id)
        && left.node_type_posteriors.len() == right.node_type_posteriors.len()
        && left
            .node_type_posteriors
            .iter()
            .zip(&right.node_type_posteriors)
            .all(|(a, b)| {
                a.pattern_id == b.pattern_id && a.node_id == b.node_id && a.type_id == b.type_id
            })
}

pub fn maximum_shift<'a>(
    baseline: impl Iterator<Item = &'a McseScalarSummary>,
    scenario: impl Iterator<Item = &'a McseScalarSummary>,
) -> f64 {
    baseline
        .zip(scenario)
        .map(|(baseline, scenario)| {
            (baseline.mean - scenario.mean).abs() / baseline.sd.max(f64::MIN_POSITIVE)
        })
        .fold(0.0_f64, f64::max)
}

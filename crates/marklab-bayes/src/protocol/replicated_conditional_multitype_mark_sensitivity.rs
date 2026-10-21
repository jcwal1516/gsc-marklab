//! Typed messages and scientific checks for replicated conditional multitype mark sensitivity.

use super::posterior_validation::summary_valid;
use crate::backend_versions::JAX_VERSION;
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::McseScalarSummary;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
pub struct SummaryRow {
    pub contrast: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct ScaleRow {
    pub scale: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct BaselineView {
    pub input_sha256: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub type_count: usize,
    pub group_affinity_shifts: Vec<SummaryRow>,
    pub patient_affinities: Vec<SummaryRow>,
    pub hierarchy_scales: Vec<ScaleRow>,
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
    pub sampling: SamplingSummary,
    pub group_affinity_shifts: Vec<SummaryRow>,
    pub patient_affinities: Vec<SummaryRow>,
    pub hierarchy_scales: Vec<ScaleRow>,
    pub diagnostics: PosteriorDiagnostics,
}

#[derive(Debug, Serialize)]
pub struct ScenarioOutput {
    pub scenario: &'static str,
    pub patient_sd_prior_scale: f64,
    pub pattern_sd_prior_scale: f64,
    pub maximum_group_affinity_shift_sd: f64,
    pub maximum_patient_affinity_shift_sd: f64,
    pub maximum_hierarchy_scale_shift_sd: f64,
    pub material_prior_sensitivity: bool,
    pub fit: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct Resources {
    pub scenario_count: usize,
    pub maximum_processes: usize,
    pub per_scenario_draw_parameter_work: u64,
    pub total_draw_parameter_work: u64,
    pub maximum_total_draw_parameter_work: u64,
    pub timeout_seconds_per_process: u64,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub format: &'static str,
    pub version: u32,
    pub baseline_request_sha256: String,
    pub scenarios: Vec<ScenarioOutput>,
    pub materiality_standard_deviations: f64,
    pub material_prior_sensitivity: bool,
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
    request_sha256: &str,
    backend: &BackendContract,
    depth: u32,
) -> Result<(), BayesError> {
    let finite = result
        .group_affinity_shifts
        .iter()
        .chain(&result.patient_affinities)
        .map(|row| &row.contrast)
        .chain(result.hierarchy_scales.iter().map(|row| &row.scale))
        .all(summary_valid);
    let complete = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= 1.01
        && result.diagnostics.ess_bulk >= 400.0
        && result.diagnostics.ess_tail >= 400.0
        && result.diagnostics.minimum_ebfmi >= 0.2
        && result.diagnostics.divergences == 0
        && result.diagnostics.max_tree_depth_hits == 0;
    if result.format != "marklab.numpyro_replicated_conditional_multitype_mark_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != baseline.input_sha256
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_request_sha256
        || result.maximum_tree_depth != depth
        || result.sampling.chains != baseline.sampling.chains
        || result.sampling.tune_per_chain != baseline.sampling.tune_per_chain
        || result.sampling.draws_per_chain != baseline.sampling.draws_per_chain
        || result.sampling.completed_draws != baseline.sampling.completed_draws
        || result.group_affinity_shifts.len() != baseline.group_affinity_shifts.len()
        || result.patient_affinities.len() != baseline.patient_affinities.len()
        || result.hierarchy_scales.len() != baseline.hierarchy_scales.len()
        || !finite
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesError::WorkerContract(
            "replicated conditional-mark sensitivity scenario differs".into(),
        ));
    }
    Ok(())
}

pub fn maximum_shift<'a>(
    baseline: impl Iterator<Item = &'a McseScalarSummary>,
    scenario: impl Iterator<Item = &'a McseScalarSummary>,
) -> f64 {
    baseline
        .zip(scenario)
        .map(|(baseline, scenario)| {
            (scenario.mean - baseline.mean).abs() / baseline.sd.max(f64::MIN_POSITIVE)
        })
        .fold(0.0, f64::max)
}

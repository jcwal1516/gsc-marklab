//! Typed messages and scientific checks for replicated arbitrary window multitype lgcp inferred kernel sbc.
use std::collections::BTreeMap;

use crate::backend_versions::JAX_VERSION;
use crate::sha256_hex;
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

pub struct ValidationContext<'a> {
    pub backend: &'a BackendContract,
    pub source_request_bytes: &'a [u8],
    pub request_sha256: &'a str,
    pub input_sha256: &'a str,
    pub calibration: &'a Calibration,
    pub resources: &'a Resources,
}

pub const SCENARIOS: [&str; 5] = [
    "positive",
    "null",
    "weak_identification",
    "boundary",
    "misspecified",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Calibration {
    pub scenarios: Vec<String>,
    pub replicates_per_scenario: u32,
    pub rank_bins: u32,
    pub interval_probability: f64,
    pub latent_node_index: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub scenario_replicates: u64,
    pub simulated_node_type_work: u64,
    pub maximum_simulated_node_type_work: u64,
    pub total_iterations: u64,
    pub maximum_total_iterations: u64,
    pub ppc_pair_work: u64,
    pub maximum_ppc_pair_work: u64,
    pub estimated_working_bytes: u64,
    pub maximum_working_bytes: u64,
    pub maximum_output_bytes: usize,
    pub timeout_seconds: u64,
}

#[derive(Serialize)]
pub struct WorkerRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: &'a str,
    pub source_request: serde_json::Value,
    pub calibration: Calibration,
    pub resources: Resources,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Truth {
    pub intercept_type_a: f64,
    pub intercept_type_b: f64,
    pub group_effect_difference: f64,
    pub patient_sd_type_a: f64,
    pub pattern_sd_type_a: f64,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub latent_node_type_a: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ranks {
    pub intercept_type_a: u64,
    pub group_effect_difference: u64,
    pub patient_sd_type_a: u64,
    pub pattern_sd_type_a: u64,
    pub field_amplitude: u64,
    pub field_length_scale_um: u64,
    pub latent_node_type_a: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub intercept_type_a: bool,
    pub group_effect_difference: bool,
    pub patient_sd_type_a: bool,
    pub pattern_sd_type_a: bool,
    pub field_amplitude: bool,
    pub field_length_scale_um: bool,
    pub latent_node_type_a: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PpcMetric {
    pub observed: f64,
    pub replicated_mean: f64,
    pub two_sided_tail_probability: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PosteriorPredictive {
    pub counts: PpcMetric,
    pub cross_type_enrichment: PpcMetric,
    pub mark_proportions: PpcMetric,
    pub clustering: PpcMetric,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioDisposition {
    pub scenario: String,
    pub replicate: u32,
    pub status: ScenarioStatus,
    pub failure_reason: Option<String>,
    pub simulated_total_events: u64,
    pub truth: Truth,
    pub ranks: Option<Ranks>,
    pub coverage_90: Option<Coverage>,
    pub posterior_predictive: Option<PosteriorPredictive>,
    pub r_hat: Option<f64>,
    pub ess_bulk: Option<f64>,
    pub ess_tail: Option<f64>,
    pub minimum_ebfmi: Option<f64>,
    pub divergences: Option<u64>,
    pub max_tree_depth_hits: Option<u64>,
}

marklab_core::vocabulary_enum! {
    /// Whether one calibration scenario completed.
    pub enum ScenarioStatus {
        Complete => "complete",
        Failed => "failed",
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterDiagnostics {
    pub completed_replicates: u64,
    pub rank_histogram: Vec<u64>,
    pub coverage_90: f64,
    pub mean_normalized_rank: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub scenario_dispositions: Vec<ScenarioDisposition>,
    pub rank_diagnostics: BTreeMap<String, ParameterDiagnostics>,
    pub pattern_representation: String,
    pub cross_type_dependence_estimand: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub calibration: Calibration,
    pub resources: Resources,
    pub scenario_dispositions: Vec<ScenarioDisposition>,
    pub rank_diagnostics: BTreeMap<String, ParameterDiagnostics>,
    pub pattern_representation: String,
    pub cross_type_dependence_estimand: String,
    pub statistical_unit: StatisticalUnit,
    pub null_model: NullHypothesis,
    pub assumptions: Vec<String>,
    pub failure_policy: String,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

impl Output {
    pub fn validate_for(&self, expected: &ValidationContext<'_>) -> Result<(), BayesError> {
        if self.format != "marklab.replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc"
            || self.version != 1
            || self.input_sha256 != expected.input_sha256
            || self.request_sha256 != expected.request_sha256
            || self.source_request_sha256 != sha256_hex(expected.source_request_bytes)
            || self.calibration != *expected.calibration
            || self.resources != *expected.resources
            || self.scenario_dispositions.len() != expected.resources.scenario_replicates as usize
            || self.pattern_representation
                != "complete_quadrature_resolved_counting_measure_on_exact_window"
            || self.cross_type_dependence_estimand
                != "not_estimated_current_model_uses_conditionally_independent_type_fields"
            || self.statistical_unit != StatisticalUnit::Patient
            || self.claim_status != ClaimStatus::ExperimentalMultitypeSbcDiagnostic
        {
            return Err(BayesError::WorkerContract(
                "durable multitype SBC result differs".into(),
            ));
        }
        Ok(())
    }
}

pub fn validate_worker(
    result: &WorkerResult,
    expected: &ValidationContext<'_>,
) -> Result<(), BayesError> {
    let per_scenario = expected.calibration.replicates_per_scenario as usize;
    let dispositions_valid = result.scenario_dispositions.len()
        == expected.resources.scenario_replicates as usize
        && result
            .scenario_dispositions
            .iter()
            .enumerate()
            .all(|(index, row)| {
                row.scenario == SCENARIOS[index / per_scenario]
                    && row.replicate == (index % per_scenario) as u32
                    && ((row.status == ScenarioStatus::Complete
                        && row.failure_reason.is_none()
                        && row.ranks.is_some()
                        && row.coverage_90.is_some()
                        && row.posterior_predictive.is_some())
                        || (row.status == ScenarioStatus::Failed
                            && row
                                .failure_reason
                                .as_ref()
                                .is_some_and(|value| !value.is_empty())))
            });
    if result.format
        != "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_result"
        || result.version != 1
        || !result.backend.matches(expected.backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != expected.input_sha256
        || result.request_sha256 != expected.request_sha256
        || result.source_request_sha256 != sha256_hex(expected.source_request_bytes)
        || result.pattern_representation
            != "complete_quadrature_resolved_counting_measure_on_exact_window"
        || result.cross_type_dependence_estimand
            != "not_estimated_current_model_uses_conditionally_independent_type_fields"
        || !dispositions_valid
    {
        return Err(BayesError::WorkerContract(
            "multitype SBC backend result differs".into(),
        ));
    }
    Ok(())
}

pub fn assumptions() -> [&'static str; 5] {
    [
        "simulation_and_refit_use_the_same_declared_quadrature_likelihood",
        "positive_null_weak_boundary_and_misspecified_scenarios_are_fixed_before_fitting",
        "type_fields_are_conditionally_independent_given_the_shared_kernel_scales",
        "cross_type_enrichment_is_a_posterior_predictive_summary_not_a_dependence_parameter",
        "patients_not_patterns_nodes_types_or_events_are_population_replicates",
    ]
}

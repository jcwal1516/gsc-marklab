//! Typed messages and scientific checks for replicated arbitrary window lgcp sbc.

use super::replicated_arbitrary_window_lgcp_fit::WorkerRequest as FitRequest;
use crate::backend_versions::JAX_VERSION;
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::NutsSamplingSpec;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Calibration {
    pub replicates: u32,
    pub rank_bins: u32,
    pub interval_probability: f64,
    pub latent_node_index: usize,
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Resources {
    pub maximum_replicates: u32,
    pub maximum_simulated_nodes: u64,
    pub maximum_total_iterations: u64,
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
    pub source_request: &'a FitRequest,
    pub calibration: Calibration,
    pub resources: Resources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Replicate {
    pub replicate: u32,
    pub simulated_total_events: u64,
    pub true_group_effect: f64,
    pub group_effect_rank: u64,
    pub group_effect_covered: bool,
    pub true_patient_sd: f64,
    pub patient_sd_rank: u64,
    pub patient_sd_covered: bool,
    pub true_pattern_sd: f64,
    pub pattern_sd_rank: u64,
    pub pattern_sd_covered: bool,
    pub true_latent_node: f64,
    pub latent_node_rank: u64,
    pub latent_node_covered: bool,
    pub r_hat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
    pub minimum_ebfmi: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub replicate: u32,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterDiagnostics {
    pub rank_histogram: Vec<u64>,
    pub rank_uniformity_p_value: f64,
    pub coverage_90: f64,
    pub mean_normalized_rank: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostics {
    pub group_effect: ParameterDiagnostics,
    pub patient_sd: ParameterDiagnostics,
    pub pattern_sd: ParameterDiagnostics,
    pub latent_node: ParameterDiagnostics,
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
    pub replicates: Vec<Replicate>,
    pub failures: Vec<Failure>,
    pub diagnostics: Diagnostics,
    pub physical_latent_pattern_id: String,
    pub physical_latent_node_id: String,
}

#[derive(Debug, Serialize)]
pub struct SbcResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub sampling: NutsSamplingSpec,
    pub calibration: Calibration,
    pub resources: Resources,
    pub replicates: Vec<Replicate>,
    pub failures: Vec<Failure>,
    pub diagnostics: Diagnostics,
    pub physical_latent_pattern_id: String,
    pub physical_latent_node_id: String,
    pub statistical_unit: StatisticalUnit,
    pub assumptions: Vec<String>,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

pub fn validate_result(
    result: &WorkerResult,
    backend: &BackendContract,
    request_sha256: &str,
    source: &FitRequest,
    source_sha256: &str,
    calibration: &Calibration,
) -> Result<(), BayesError> {
    let completed_draws = source.completed_draws();
    let (pattern_id, node_id) = source.first_node_identity();
    let completed = result.replicates.len();
    let dispositions_complete =
        completed + result.failures.len() == calibration.replicates as usize;
    let disposition_indices = result
        .replicates
        .iter()
        .map(|row| row.replicate)
        .chain(result.failures.iter().map(|row| row.replicate))
        .collect::<std::collections::BTreeSet<_>>();
    let dispositions_exact = disposition_indices
        == (0..calibration.replicates).collect::<std::collections::BTreeSet<_>>()
        && result
            .failures
            .iter()
            .all(|failure| !failure.reason.is_empty());
    let replicate_rows_valid = result.replicates.iter().all(|row| {
        let finite = [
            row.true_group_effect,
            row.true_patient_sd,
            row.true_pattern_sd,
            row.true_latent_node,
            row.r_hat,
            row.ess_bulk,
            row.ess_tail,
            row.minimum_ebfmi,
        ]
        .into_iter()
        .all(f64::is_finite);
        finite
            && row.replicate < calibration.replicates
            && row.group_effect_rank <= completed_draws
            && row.patient_sd_rank <= completed_draws
            && row.pattern_sd_rank <= completed_draws
            && row.latent_node_rank <= completed_draws
    });
    let parameters = [
        &result.diagnostics.group_effect,
        &result.diagnostics.patient_sd,
        &result.diagnostics.pattern_sd,
        &result.diagnostics.latent_node,
    ];
    let diagnostic_rows_valid = parameters.iter().all(|diagnostic| {
        diagnostic.rank_histogram.len() == calibration.rank_bins as usize
            && diagnostic.rank_histogram.iter().sum::<u64>() == completed as u64
            && [
                diagnostic.rank_uniformity_p_value,
                diagnostic.coverage_90,
                diagnostic.mean_normalized_rank,
            ]
            .into_iter()
            .all(f64::is_finite)
            && (0.0..=1.0).contains(&diagnostic.rank_uniformity_p_value)
            && (0.0..=1.0).contains(&diagnostic.coverage_90)
            && (0.0..=1.0).contains(&diagnostic.mean_normalized_rank)
    });
    let passes = result.failures.is_empty()
        && parameters.iter().all(|diagnostic| {
            diagnostic.rank_uniformity_p_value >= calibration.minimum_rank_uniformity_p_value
                && (calibration.minimum_coverage..=calibration.maximum_coverage)
                    .contains(&diagnostic.coverage_90)
        });
    if result.format != "marklab.numpyro_replicated_arbitrary_window_lgcp_sbc_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != source.input_sha256()
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_sha256
        || result.physical_latent_pattern_id != pattern_id
        || result.physical_latent_node_id != node_id
        || !dispositions_complete
        || !dispositions_exact
        || !replicate_rows_valid
        || !diagnostic_rows_valid
        || (result.fit_state == FitState::Complete) != passes
    {
        return Err(BayesError::WorkerContract(
            "replicated LGCP SBC result differs".into(),
        ));
    }
    Ok(())
}

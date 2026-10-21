//! Typed messages and scientific checks for replicated conditional multitype mark sbc.

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
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize)]
pub struct Calibration {
    pub replicates: u32,
    pub rank_bins: u32,
    pub interval_probability: f64,
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Resources {
    pub maximum_states_per_pattern: u64,
    pub states_per_pattern: u64,
    pub points_per_pattern: usize,
    pub pattern_count: usize,
    pub maximum_enumeration_work: u64,
    pub enumeration_work: u64,
    pub maximum_enumeration_bytes: u64,
    pub enumeration_bytes: u64,
    pub maximum_total_iterations: u64,
    pub total_iterations: u64,
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicateParameter {
    pub parameter: String,
    pub truth: f64,
    pub rank: u64,
    pub covered: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Replicate {
    pub replicate: u32,
    pub simulated_pattern_type_counts: Vec<Vec<u64>>,
    pub parameters: Vec<ReplicateParameter>,
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
pub struct Diagnostic {
    pub parameter: String,
    pub rank_histogram: Vec<u64>,
    pub rank_uniformity_p_value: f64,
    pub coverage_90: f64,
    pub mean_normalized_rank: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnumerationOracle {
    pub states_per_pattern: u64,
    pub zero_parameter_log_normalizer_per_pattern: f64,
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
    pub enumeration_oracle: EnumerationOracle,
    pub replicates: Vec<Replicate>,
    pub failures: Vec<Failure>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub format: &'static str,
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
    pub enumeration_oracle: EnumerationOracle,
    pub replicates: Vec<Replicate>,
    pub failures: Vec<Failure>,
    pub diagnostics: Vec<Diagnostic>,
    pub statistical_unit: StatisticalUnit,
    pub generative_model: &'static str,
    pub fitted_model: &'static str,
    pub assumptions: [&'static str; 5],
    pub finite_result_policy: &'static str,
    pub claim_status: ClaimStatus,
}

/// Request-side values a worker result must reproduce.
#[derive(Clone, Copy)]
pub struct ValidationContext<'a> {
    pub backend: &'a BackendContract,
    pub request_sha256: &'a str,
    pub source_request_sha256: &'a str,
    pub input_sha256: &'a str,
    pub types: &'a [String],
    pub pattern_count: usize,
    pub points_per_pattern: usize,
    pub sampling: &'a NutsSamplingSpec,
    pub calibration: &'a Calibration,
}

pub fn validate_result(
    result: &WorkerResult,
    context: ValidationContext<'_>,
) -> Result<(), BayesError> {
    let ValidationContext {
        backend,
        request_sha256: request_sha,
        source_request_sha256: source_sha,
        input_sha256: input_sha,
        types,
        pattern_count,
        points_per_pattern,
        sampling,
        calibration,
    } = context;
    let names = parameter_names(types);
    let completed_draws = u64::from(sampling.chains) * u64::from(sampling.draws_per_chain);
    let dispositions = result
        .replicates
        .iter()
        .map(|row| row.replicate)
        .chain(result.failures.iter().map(|row| row.replicate))
        .collect::<BTreeSet<_>>();
    let disposition_valid = result.replicates.len() + result.failures.len()
        == calibration.replicates as usize
        && dispositions == (0..calibration.replicates).collect::<BTreeSet<_>>()
        && result.failures.iter().all(|row| !row.reason.is_empty());
    let rows_valid = result.replicates.iter().all(|row| {
        row.simulated_pattern_type_counts.len() == pattern_count
            && row.simulated_pattern_type_counts.iter().all(|counts| {
                counts.len() == types.len()
                    && counts.iter().sum::<u64>() == points_per_pattern as u64
            })
            && row.parameters.len() == names.len()
            && row.parameters.iter().zip(&names).all(|(parameter, name)| {
                parameter.parameter == *name
                    && parameter.truth.is_finite()
                    && parameter.rank <= completed_draws
            })
            && [row.r_hat, row.ess_bulk, row.ess_tail, row.minimum_ebfmi]
                .into_iter()
                .all(f64::is_finite)
    });
    let diagnostics_valid = result.diagnostics.len() == names.len()
        && result.diagnostics.iter().zip(&names).all(|(row, name)| {
            row.parameter == *name
                && row.rank_histogram.len() == calibration.rank_bins as usize
                && row.rank_histogram.iter().sum::<u64>() == result.replicates.len() as u64
                && [
                    row.rank_uniformity_p_value,
                    row.coverage_90,
                    row.mean_normalized_rank,
                ]
                .into_iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
        });
    let passes = result.failures.is_empty()
        && result.diagnostics.iter().all(|row| {
            row.rank_uniformity_p_value >= calibration.minimum_rank_uniformity_p_value
                && (calibration.minimum_coverage..=calibration.maximum_coverage)
                    .contains(&row.coverage_90)
        });
    let expected_states = (types.len() as u64).pow(points_per_pattern as u32);
    let expected_zero_log_normalizer = points_per_pattern as f64 * (types.len() as f64).ln();
    if result.format != "marklab.numpyro_replicated_conditional_multitype_mark_sbc_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_sha
        || result.input_sha256 != input_sha
        || result.enumeration_oracle.states_per_pattern != expected_states
        || (result
            .enumeration_oracle
            .zero_parameter_log_normalizer_per_pattern
            - expected_zero_log_normalizer)
            .abs()
            > 1e-12
        || !disposition_valid
        || !rows_valid
        || !diagnostics_valid
        || (result.fit_state == FitState::Complete) != passes
    {
        return Err(BayesError::WorkerContract(
            "replicated conditional-mark SBC result differs".into(),
        ));
    }
    Ok(())
}

fn parameter_names(types: &[String]) -> Vec<String> {
    let mut names = Vec::new();
    for prefix in ["baseline_affinity", "group_affinity_shift"] {
        for left in 0..types.len() {
            for right in left + 1..types.len() {
                names.push(format!("{prefix}:{}|{}", types[left], types[right]));
            }
        }
    }
    names.extend([
        "patient_intercept_sd".into(),
        "patient_potential_sd".into(),
        "pattern_intercept_sd".into(),
        "pattern_potential_sd".into(),
    ]);
    names
}

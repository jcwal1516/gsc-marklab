//! Typed messages and scientific checks for conditional multitype mark sbc.

use super::conditional_mark_sbc_checks as checks;
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
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Resources {
    pub maximum_states: u64,
    pub state_count: u64,
    pub maximum_enumeration_work: u64,
    pub enumeration_work: u64,
    pub maximum_enumeration_bytes: u64,
    pub enumeration_bytes: u64,
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
    pub simulated_type_counts: Vec<u64>,
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
    pub state_count: u64,
    pub zero_parameter_log_normalizer: f64,
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
    pub assumptions: [&'static str; 4],
    pub finite_result_policy: &'static str,
    pub claim_status: ClaimStatus,
}

/// Request-side values a worker result must reproduce.
#[derive(Clone, Copy)]
pub struct ValidationContext<'a> {
    pub backend: &'a BackendContract,
    pub request_sha256: &'a str,
    pub source_request_sha256: &'a str,
    pub type_ids: &'a [String],
    pub point_count: usize,
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
        type_ids,
        point_count,
        sampling,
        calibration,
    } = context;
    let names = parameter_names(type_ids);
    let completed_draws = u64::from(sampling.chains) * u64::from(sampling.draws_per_chain);
    let dispositions_valid = checks::dispositions_valid(
        result.replicates.iter().map(|row| row.replicate),
        result
            .failures
            .iter()
            .map(|row| (row.replicate, row.reason.as_str())),
        calibration.replicates,
    );
    let rows_valid = result.replicates.iter().all(|row| {
        row.simulated_type_counts.len() == type_ids.len()
            && row.simulated_type_counts.iter().sum::<u64>() == point_count as u64
            && checks::parameters_valid(
                row.parameters.iter().map(|parameter| {
                    (
                        parameter.parameter.as_str(),
                        parameter.truth,
                        parameter.rank,
                    )
                }),
                &names,
                completed_draws,
            )
            && [row.r_hat, row.ess_bulk, row.ess_tail, row.minimum_ebfmi]
                .into_iter()
                .all(f64::is_finite)
    });
    let diagnostics_valid = checks::diagnostics_valid(
        result.diagnostics.iter().map(|row| {
            (
                row.parameter.as_str(),
                row.rank_histogram.as_slice(),
                [
                    row.rank_uniformity_p_value,
                    row.coverage_90,
                    row.mean_normalized_rank,
                ],
            )
        }),
        &names,
        calibration.rank_bins,
        result.replicates.len(),
    );
    let passes = result.failures.is_empty()
        && checks::calibration_passes(
            result
                .diagnostics
                .iter()
                .map(|row| (row.rank_uniformity_p_value, row.coverage_90)),
            calibration.minimum_rank_uniformity_p_value,
            calibration.minimum_coverage,
            calibration.maximum_coverage,
        );
    if result.format != "marklab.numpyro_conditional_multitype_mark_sbc_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_sha
        || !checks::enumeration_valid(
            result.enumeration_oracle.state_count,
            result.enumeration_oracle.zero_parameter_log_normalizer,
            type_ids.len(),
            point_count,
        )
        || !dispositions_valid
        || !rows_valid
        || !diagnostics_valid
        || (result.fit_state == FitState::Complete) != passes
    {
        return Err(BayesError::WorkerContract(
            "conditional multitype SBC result differs".into(),
        ));
    }
    Ok(())
}

fn parameter_names(type_ids: &[String]) -> Vec<String> {
    let mut names = type_ids[1..]
        .iter()
        .map(|identity| format!("intercept:{identity}"))
        .collect::<Vec<_>>();
    for left in 0..type_ids.len() {
        for right in left..type_ids.len() {
            if left != 0 || right != 0 {
                names.push(format!("potential:{}|{}", type_ids[left], type_ids[right]));
            }
        }
    }
    for left in 0..type_ids.len() {
        for right in left + 1..type_ids.len() {
            names.push(format!("affinity:{}|{}", type_ids[left], type_ids[right]));
        }
    }
    names
}

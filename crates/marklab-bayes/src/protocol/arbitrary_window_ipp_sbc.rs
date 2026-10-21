//! Typed messages and scientific checks for arbitrary window ipp sbc.

use crate::backend_versions::JAX_VERSION;
use crate::BackendContract;
use crate::BayesError;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Serialize)]
pub struct SbcRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: &'a str,
    pub source_request: &'a crate::InhomogeneousPoissonFitWorkerRequest,
    pub replicates: u32,
    pub maximum_generated_count: u64,
    pub maximum_total_work: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationSummary {
    replicates: usize,
    posterior_draws_per_replicate: u64,
    normalized_mean_rank: f64,
    coverage_90: f64,
    accepted: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SbcReplicate {
    replicate: u32,
    true_intercept: f64,
    true_coefficient: f64,
    true_total_expected_count: f64,
    realized_total_count: u64,
    intercept_rank: u64,
    coefficient_rank: u64,
    total_expected_count_rank: u64,
    intercept_covered_90: bool,
    coefficient_covered_90: bool,
    total_expected_count_covered_90: bool,
    r_hat: f64,
    divergences: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SbcFailure {
    replicate: u32,
    reason: String,
    true_total_expected_count: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SbcResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    requested_replicates: usize,
    completed_replicates: usize,
    failed_replicates: usize,
    replicates: Vec<SbcReplicate>,
    failures: Vec<SbcFailure>,
    calibration: BTreeMap<String, CalibrationSummary>,
    calibration_status: String,
    total_work: u64,
    maximum_generated_count: u64,
    claim_status: ClaimStatus,
}

pub fn validate(
    result: &SbcResult,
    request: &SbcRequest<'_>,
    request_sha256: &str,
    quadrature_node_count: usize,
) -> Result<(), BayesError> {
    let names = ["intercept", "coefficient", "total_expected_count"];
    let expected_draws = u64::from(request.source_request.sampling.chains)
        * u64::from(request.source_request.sampling.draws_per_chain);
    let expected_work = u64::from(request.replicates)
        * u64::from(request.source_request.sampling.chains)
        * u64::from(
            request.source_request.sampling.tune_per_chain
                + request.source_request.sampling.draws_per_chain,
        )
        * quadrature_node_count as u64;
    let calibration_valid = names.iter().all(|name| {
        result.calibration.get(*name).is_some_and(|value| {
            value.replicates == result.completed_replicates
                && value.posterior_draws_per_replicate == expected_draws
                && value.normalized_mean_rank.is_finite()
                && (0.0..=1.0).contains(&value.normalized_mean_rank)
                && value.coverage_90.is_finite()
                && (0.0..=1.0).contains(&value.coverage_90)
                && value.accepted
                    == ((0.25..=0.75).contains(&value.normalized_mean_rank)
                        && (0.65..=1.0).contains(&value.coverage_90))
        })
    });
    let replicates_valid = result.replicates.iter().all(|value| {
        [
            value.true_intercept,
            value.true_coefficient,
            value.true_total_expected_count,
            value.r_hat,
        ]
        .into_iter()
        .all(f64::is_finite)
            && value.true_total_expected_count > 0.0
            && value.intercept_rank <= expected_draws
            && value.coefficient_rank <= expected_draws
            && value.total_expected_count_rank <= expected_draws
    });
    let failures_valid = result.failures.iter().all(|value| {
        matches!(
            value.reason.as_str(),
            "generated_count_resource_ceiling"
                | "realized_count_resource_ceiling"
                | "non_finite_posterior"
        ) && value
            .true_total_expected_count
            .is_none_or(|count| count.is_finite() && count > 0.0)
    });
    let accepted = result.failed_replicates == 0
        && result.completed_replicates == result.requested_replicates
        && names.iter().all(|name| result.calibration[*name].accepted)
        && result
            .replicates
            .iter()
            .all(|value| value.divergences == 0 && value.r_hat <= 1.05);
    if result.format != "marklab.arbitrary_window_ipp_sbc"
        || result.version != 1
        || !result.backend.matches(&request.backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != request.source_request_sha256
        || result.requested_replicates != request.replicates as usize
        || result.completed_replicates != result.replicates.len()
        || result.failed_replicates != result.failures.len()
        || result.completed_replicates + result.failed_replicates != result.requested_replicates
        || result.calibration.len() != names.len()
        || !calibration_valid
        || !replicates_valid
        || !failures_valid
        || result.total_work != expected_work
        || result.total_work > request.maximum_total_work
        || result.maximum_generated_count != request.maximum_generated_count
        || (result.calibration_status == "accepted") != accepted
        || !matches!(
            result.calibration_status.as_str(),
            "accepted" | "not_accepted"
        )
        || result.claim_status != ClaimStatus::SyntheticPriorGenerativeCalibrationOnly
    {
        return Err(BayesError::WorkerContract(
            "weighted IPP SBC identity, bounds, or calibration contract differs".into(),
        ));
    }
    Ok(())
}

//! Typed messages and scientific checks for arbitrary window ipp spatial ppc.
use super::posterior_validation::diagnostics_pass;
use crate::InhomogeneousPoissonFitWorkerRequest;

use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct SpatialPpcRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub source_request_sha256: String,
    pub source_request: crate::InhomogeneousPoissonFitWorkerRequest,
    pub input: SpatialPpcInputIdentity,
    pub observed_node_counts: Vec<u64>,
    pub node_weights_um2: Vec<f64>,
    pub neighbor_pairs: Vec<[usize; 2]>,
    pub neighbor_radius_um: f64,
    pub prediction_seed: u64,
    pub maximum_predictive_work: u64,
    pub maximum_neighbor_pairs: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialPpcInputIdentity {
    pub events_digest: String,
    pub event_membership_digest: String,
    pub quadrature_digest: String,
    pub window_logical_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialPpcSummary {
    observed: f64,
    replicated_mean: f64,
    replicated_sd: f64,
    probability_replicated_at_least_observed: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialPpcSummaries {
    node_density_variance: SpatialPpcSummary,
    neighbor_density_mean_absolute_difference: SpatialPpcSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialPpcResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    input: SpatialPpcInputIdentity,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    diagnostics: PosteriorDiagnostics,
    observed_event_count: usize,
    quadrature_node_count: usize,
    neighbor_radius_um: f64,
    neighbor_pair_count: usize,
    posterior_predictive_replicates: u64,
    prediction_seed: u64,
    total_predictive_work: u64,
    maximum_predictive_work: u64,
    maximum_neighbor_pairs: usize,
    summaries: SpatialPpcSummaries,
    statistical_unit: StatisticalUnit,
    null_model: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: ClaimStatus,
}

pub fn validate(
    request: &SpatialPpcRequest,
    request_sha256: &str,
    source_request: &InhomogeneousPoissonFitWorkerRequest,
    source_request_sha256: &str,
    observed_event_count: usize,
    quadrature_node_count: usize,
    result: &SpatialPpcResult,
) -> Result<(), BayesError> {
    let draws = u64::from(source_request.sampling.chains)
        * u64::from(source_request.sampling.draws_per_chain);
    let expected_work = draws * (quadrature_node_count + request.neighbor_pairs.len()) as u64;
    let summary_valid = |summary: &SpatialPpcSummary| {
        [
            summary.observed,
            summary.replicated_mean,
            summary.replicated_sd,
            summary.probability_replicated_at_least_observed,
        ]
        .into_iter()
        .all(f64::is_finite)
            && summary.observed >= 0.0
            && summary.replicated_mean >= 0.0
            && summary.replicated_sd >= 0.0
            && (0.0..=1.0).contains(&summary.probability_replicated_at_least_observed)
    };
    let diagnostics_finite = [
        result.diagnostics.r_hat,
        result.diagnostics.ess_bulk,
        result.diagnostics.ess_tail,
        result.diagnostics.mcse_mean,
        result.diagnostics.mcse_sd,
        result.diagnostics.minimum_ebfmi,
    ]
    .into_iter()
    .all(f64::is_finite);
    let diagnostics_pass = diagnostics_finite
        && diagnostics_pass(&result.diagnostics, &source_request.diagnostic_policy);
    if result.format != "marklab.arbitrary_window_ipp_spatial_ppc"
        || result.version != 1
        || !result.backend.matches(&request.backend)
        || result.input.events_digest != request.input.events_digest
        || result.input.event_membership_digest != request.input.event_membership_digest
        || result.input.quadrature_digest != request.input.quadrature_digest
        || result.input.window_logical_digest != request.input.window_logical_digest
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_request_sha256
        || result.sampling.chains != source_request.sampling.chains
        || result.sampling.tune_per_chain != source_request.sampling.tune_per_chain
        || result.sampling.draws_per_chain != source_request.sampling.draws_per_chain
        || result.sampling.completed_draws != draws
        || (result.fit_state == FitState::Complete) != diagnostics_pass
        || result.observed_event_count != observed_event_count
        || result.quadrature_node_count != quadrature_node_count
        || !equal(result.neighbor_radius_um, request.neighbor_radius_um)
        || result.neighbor_pair_count != request.neighbor_pairs.len()
        || result.posterior_predictive_replicates != draws
        || result.prediction_seed != request.prediction_seed
        || result.total_predictive_work != expected_work
        || result.total_predictive_work > result.maximum_predictive_work
        || result.maximum_predictive_work != request.maximum_predictive_work
        || result.maximum_neighbor_pairs != request.maximum_neighbor_pairs
        || !summary_valid(&result.summaries.node_density_variance)
        || !summary_valid(&result.summaries.neighbor_density_mean_absolute_difference)
        || result.statistical_unit != StatisticalUnit::OneObservedPointPattern
        || result.null_model
            != NullHypothesis::FittedLogLinearInhomogeneousPoissonProcessConditionalOnFixedCovariate
                .as_str()
        || result.assumptions
            != [
                "event_to_quadrature_membership_is_exact_and_complete",
                "quadrature_weights_partition_the_exact_window_area",
                "neighbor_pairs_use_fixed_physical_representative_distance",
                "node_density_is_count_per_square_micrometer",
            ]
        || result.finite_result_policy != "reject_non_finite_nonconverged_is_diagnostic_only"
        || (result.fit_state == FitState::Complete
            && result.claim_status != ClaimStatus::ExperimentalSinglePatternSpatialPpc)
        || (result.fit_state != FitState::Complete
            && result.claim_status != ClaimStatus::DiagnosticOnlyNonconverged)
    {
        return Err(BayesError::WorkerContract(
            "arbitrary-window IPP spatial PPC identity, bounds, or result differs".into(),
        ));
    }
    Ok(())
}

fn equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12 * left.abs().max(right.abs()).max(1.0)
}

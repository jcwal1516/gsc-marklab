//! Typed messages and scientific checks for conditional multitype mark agreement.

use super::conditional_multitype_mark::Output as FitOutput;
use super::posterior_validation::summary_valid;
use super::replicated_arbitrary_window_lgcp_agreement::compare_scalar;
use super::replicated_arbitrary_window_lgcp_agreement::compare_vector;
use super::replicated_arbitrary_window_lgcp_agreement::ScalarAgreement;
use super::replicated_arbitrary_window_lgcp_agreement::VectorAgreement;
use crate::backend_versions::JAX_VERSION;
use crate::AgreementStatus;
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

#[derive(Debug, Deserialize, Serialize)]
pub struct TypeRow {
    pub type_id: String,
    pub intercept: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PotentialRow {
    pub type_a: String,
    pub type_b: String,
    pub potential: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffinityRow {
    pub type_a: String,
    pub type_b: String,
    pub contrast: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScoreRows {
    pub independent_label_composite_log_score: f64,
    pub spatial_composite_log_score: McseScalarSummary,
    pub mean_composite_log_score_improvement: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PpcRows {
    pub mode: String,
    pub observed_same_type_edges: u64,
    pub expected_same_type_edges: McseScalarSummary,
    pub observed_type_counts: Vec<u64>,
    pub expected_type_counts: Vec<McseScalarSummary>,
}

#[derive(Debug, Deserialize)]
pub struct FitView {
    pub input_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub type_intercepts: Vec<TypeRow>,
    pub pair_potentials: Vec<PotentialRow>,
    pub pair_affinity_contrasts: Vec<AffinityRow>,
    pub comparison: ScoreRows,
    pub posterior_predictive: PpcRows,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroResult {
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
    pub type_intercepts: Vec<TypeRow>,
    pub pair_potentials: Vec<PotentialRow>,
    pub pair_affinity_contrasts: Vec<AffinityRow>,
    pub comparison: ScoreRows,
    pub posterior_predictive: PpcRows,
    pub diagnostics: PosteriorDiagnostics,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub intercepts: VectorAgreement,
    pub pair_potentials: VectorAgreement,
    pub pair_affinities: VectorAgreement,
    pub composite_log_score: ScalarAgreement,
    pub expected_same_type_edges: ScalarAgreement,
    pub expected_type_counts: VectorAgreement,
    pub all_pass: bool,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub format: &'static str,
    pub version: u32,
    pub pymc: FitOutput,
    pub numpyro: NumpyroResult,
    pub comparison: Comparison,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub statistical_unit: StatisticalUnit,
    pub assumptions: [&'static str; 4],
    pub finite_result_policy: &'static str,
    pub claim_status: ClaimStatus,
}

pub fn validate_numpyro(
    result: &NumpyroResult,
    pymc: &FitView,
    source_request_sha256: &str,
    request_sha: &str,
    backend: &BackendContract,
    depth: u32,
) -> Result<(), BayesError> {
    let rows_valid = result
        .type_intercepts
        .iter()
        .all(|row| summary_valid(&row.intercept))
        && result
            .pair_potentials
            .iter()
            .all(|row| summary_valid(&row.potential))
        && result
            .pair_affinity_contrasts
            .iter()
            .all(|row| summary_valid(&row.contrast))
        && summary_valid(&result.comparison.spatial_composite_log_score)
        && summary_valid(&result.posterior_predictive.expected_same_type_edges)
        && result
            .posterior_predictive
            .expected_type_counts
            .iter()
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
    if result.format != "marklab.numpyro_conditional_multitype_mark_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != pymc.input_sha256
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_request_sha256
        || result.maximum_tree_depth != depth
        || result.sampling.completed_draws != pymc.sampling.completed_draws
        || result.type_intercepts.len() != pymc.type_intercepts.len()
        || result.pair_potentials.len() != pymc.pair_potentials.len()
        || result.pair_affinity_contrasts.len() != pymc.pair_affinity_contrasts.len()
        || result.posterior_predictive.mode != "one_step_conditionals_given_observed_neighbors"
        || result.posterior_predictive.observed_same_type_edges
            != pymc.posterior_predictive.observed_same_type_edges
        || result.posterior_predictive.observed_type_counts
            != pymc.posterior_predictive.observed_type_counts
        || !rows_valid
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesError::WorkerContract(
            "conditional multitype NumPyro result differs".into(),
        ));
    }
    Ok(())
}

pub fn compare(
    left: &FitView,
    right: &NumpyroResult,
    standardized: f64,
    tolerance: f64,
) -> Comparison {
    let intercepts = compare_vector(
        left.type_intercepts.iter().map(|row| &row.intercept),
        right.type_intercepts.iter().map(|row| &row.intercept),
        standardized,
        tolerance,
    );
    let pair_potentials = compare_vector(
        left.pair_potentials.iter().map(|row| &row.potential),
        right.pair_potentials.iter().map(|row| &row.potential),
        standardized,
        tolerance,
    );
    let pair_affinities = compare_vector(
        left.pair_affinity_contrasts.iter().map(|row| &row.contrast),
        right
            .pair_affinity_contrasts
            .iter()
            .map(|row| &row.contrast),
        standardized,
        tolerance,
    );
    let composite_log_score = compare_scalar(
        &left.comparison.spatial_composite_log_score,
        &right.comparison.spatial_composite_log_score,
        standardized,
        tolerance,
    );
    let expected_same_type_edges = compare_scalar(
        &left.posterior_predictive.expected_same_type_edges,
        &right.posterior_predictive.expected_same_type_edges,
        standardized,
        tolerance,
    );
    let expected_type_counts = compare_vector(
        left.posterior_predictive.expected_type_counts.iter(),
        right.posterior_predictive.expected_type_counts.iter(),
        standardized,
        tolerance,
    );
    let all_pass = intercepts.passes
        && pair_potentials.passes
        && pair_affinities.passes
        && composite_log_score.passes
        && expected_same_type_edges.passes
        && expected_type_counts.passes;
    Comparison {
        intercepts,
        pair_potentials,
        pair_affinities,
        composite_log_score,
        expected_same_type_edges,
        expected_type_counts,
        all_pass,
    }
}

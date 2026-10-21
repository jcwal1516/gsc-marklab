//! Conditional pseudolikelihood for hard multitype marks on one fixed-location pattern.
//!
//! Locations are conditioned on; edges join point pairs at Euclidean distance `<= radius_um`.
//! With `counts_il` the number of type-`l` neighbours of point `i`, each label follows
//! `P(y_i = k | rest) = softmax_k(intercept_k + sum_l counts_il potential_kl)`, and the
//! likelihood is the product over points: a composite, not a normalized joint likelihood. The
//! first type is `reference_type`, with `intercept_0 = 0` and `potential_00 = 0`; the potential
//! matrix is symmetric, leaving `T - 1` free intercepts and `T (T + 1) / 2 - 1` free potentials
//! with priors `Normal(0, intercept_sd)` and `Normal(0, interaction_sd)`. Affinity contrasts are
//! `potential_ab - (potential_aa + potential_bb) / 2`. `fit_state` requires finite summaries and
//! R-hat at most 1.01, bulk and tail ESS at least 400, E-BFMI at least 0.2, and no divergences or
//! depth hits. The log-score gain is in-sample, against independent labels at the observed type
//! proportions, and the experimental claim does not depend on `fit_state`.

use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::McseScalarSummary;
use crate::NutsSamplingSpec;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub struct CsvPoint {
    #[serde(alias = "cell_id")]
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
    #[serde(alias = "histologic_compartment")]
    pub type_id: String,
}

#[derive(Debug, Serialize)]
pub struct PointRow {
    pub point_id: String,
    pub type_index: usize,
    pub neighbor_counts: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct Edge {
    pub left: usize,
    pub right: usize,
}

#[derive(Debug, Serialize)]
pub struct Priors {
    pub intercept_sd: f64,
    pub interaction_sd: f64,
}

#[derive(Debug, Serialize)]
pub struct DiagnosticPolicy {
    pub maximum_r_hat: f64,
    pub minimum_bulk_ess: f64,
    pub minimum_tail_ess: f64,
    pub minimum_ebfmi: f64,
    pub maximum_divergences: u64,
    pub maximum_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub maximum_points: usize,
    pub maximum_types: usize,
    pub maximum_neighbor_visits: u64,
    pub neighbor_visits: u64,
    pub maximum_edges: usize,
    pub maximum_draw_parameter_work: u64,
    pub draw_parameter_work: u64,
    pub maximum_working_bytes: usize,
    pub estimated_working_bytes: usize,
    pub memory_scope: String,
    pub maximum_output_bytes: usize,
    pub maximum_tree_depth: u32,
    pub timeout_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct WorkerRequest {
    pub format: String,
    pub version: u32,
    pub backend: BackendContract,
    pub input_sha256: String,
    pub type_ids: Vec<String>,
    pub reference_type: String,
    pub radius_um: f64,
    pub points: Vec<PointRow>,
    pub edges: Vec<Edge>,
    pub priors: Priors,
    pub sampling: NutsSamplingSpec,
    pub diagnostic_policy: DiagnosticPolicy,
    pub resources: Resources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeSummary {
    pub type_id: String,
    pub intercept: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PotentialSummary {
    pub type_a: String,
    pub type_b: String,
    pub potential: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AffinitySummary {
    pub type_a: String,
    pub type_b: String,
    pub contrast: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub independent_label_composite_log_score: f64,
    pub spatial_composite_log_score: McseScalarSummary,
    pub mean_composite_log_score_improvement: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PosteriorPredictive {
    pub mode: String,
    pub observed_same_type_edges: u64,
    pub expected_same_type_edges: McseScalarSummary,
    pub observed_type_counts: Vec<u64>,
    pub expected_type_counts: Vec<McseScalarSummary>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub input_sha256: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub type_intercepts: Vec<TypeSummary>,
    pub pair_potentials: Vec<PotentialSummary>,
    pub pair_affinity_contrasts: Vec<AffinitySummary>,
    pub comparison: Comparison,
    pub posterior_predictive: PosteriorPredictive,
    pub diagnostics: PosteriorDiagnostics,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub input_sha256: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub radius_um: f64,
    pub type_ids: Vec<String>,
    pub reference_type: String,
    pub point_count: usize,
    pub type_count: usize,
    pub edge_count: usize,
    pub type_intercepts: Vec<TypeSummary>,
    pub pair_potentials: Vec<PotentialSummary>,
    pub pair_affinity_contrasts: Vec<AffinitySummary>,
    pub comparison: Comparison,
    pub posterior_predictive: PosteriorPredictive,
    pub diagnostics: PosteriorDiagnostics,
    pub resources: Resources,
    pub statistical_unit: StatisticalUnit,
    pub null_model: NullHypothesis,
    pub assumptions: [String; 4],
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

impl Output {
    pub fn validate_for(
        &self,
        input_sha256: &str,
        reference_type: &str,
        radius_um: f64,
        backend: &BackendContract,
    ) -> Result<(), BayesError> {
        let complete = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= 1.01
            && self.diagnostics.ess_bulk >= 400.0
            && self.diagnostics.ess_tail >= 400.0
            && self.diagnostics.minimum_ebfmi >= 0.2
            && self.diagnostics.divergences == 0
            && self.diagnostics.max_tree_depth_hits == 0;
        let expected_potentials = self.type_count * (self.type_count + 1) / 2;
        let expected_affinities = self.type_count * (self.type_count - 1) / 2;
        if self.format != "marklab.conditional_multitype_mark_fit"
            || self.version != 1
            || !self.backend.matches(backend)
            || self.input_sha256 != input_sha256
            || self.reference_type != reference_type
            || self.type_ids.first().map(String::as_str) != Some(reference_type)
            || self.radius_um.to_bits() != radius_um.to_bits()
            || self.point_count < 6
            || self.type_count != self.type_ids.len()
            || self.type_intercepts.len() != self.type_count
            || self.pair_potentials.len() != expected_potentials
            || self.pair_affinity_contrasts.len() != expected_affinities
            || self.posterior_predictive.mode != "one_step_conditionals_given_observed_neighbors"
            || self.posterior_predictive.observed_type_counts.len() != self.type_count
            || self.posterior_predictive.expected_type_counts.len() != self.type_count
            || (self.fit_state == FitState::Complete) != complete
        {
            return Err(BayesError::WorkerContract(
                "conditional multitype durable result differs".into(),
            ));
        }
        Ok(())
    }
}

pub fn validate_points(points: &[CsvPoint]) -> Result<(), BayesError> {
    let mut ids = HashSet::new();
    let mut coordinates = HashSet::new();
    for point in points {
        if point.point_id.is_empty()
            || point.point_id.trim() != point.point_id
            || point.type_id.is_empty()
            || point.type_id.trim() != point.type_id
            || ![point.x_um, point.y_um].into_iter().all(f64::is_finite)
            || !ids.insert(point.point_id.as_str())
            || !coordinates.insert((point.x_um.to_bits(), point.y_um.to_bits()))
        {
            return Err(BayesError::InvalidSpec(
                "conditional multitype points require unique exact IDs/coordinates and finite values"
                    .into(),
            ));
        }
    }
    Ok(())
}

pub fn validate_result(
    result: &WorkerResult,
    request: &WorkerRequest,
    backend: &BackendContract,
    request_sha: &str,
) -> Result<(), BayesError> {
    let summary_valid = |summary: &McseScalarSummary| {
        [
            summary.mean,
            summary.sd,
            summary.interval_lower,
            summary.interval_upper,
        ]
        .into_iter()
        .all(f64::is_finite)
            && summary.sd >= 0.0
            && summary.interval_lower <= summary.interval_upper
    };
    let diagnostics = &result.diagnostics;
    let complete = diagnostics.prior_predictive_finite
        && diagnostics.posterior_finite
        && diagnostics.constraints_valid
        && diagnostics.identifiability_checks_passed
        && diagnostics.r_hat <= request.diagnostic_policy.maximum_r_hat
        && diagnostics.ess_bulk >= request.diagnostic_policy.minimum_bulk_ess
        && diagnostics.ess_tail >= request.diagnostic_policy.minimum_tail_ess
        && diagnostics.minimum_ebfmi >= request.diagnostic_policy.minimum_ebfmi
        && diagnostics.divergences <= request.diagnostic_policy.maximum_divergences
        && diagnostics.max_tree_depth_hits <= request.diagnostic_policy.maximum_tree_depth_hits;
    let expected_potentials = request.type_ids.len() * (request.type_ids.len() + 1) / 2;
    let expected_affinities = request.type_ids.len() * (request.type_ids.len() - 1) / 2;
    let summaries_valid = result
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
    let finite = [
        result.comparison.independent_label_composite_log_score,
        result.comparison.mean_composite_log_score_improvement,
    ]
    .into_iter()
    .all(f64::is_finite);
    if result.format != "marklab.pymc_conditional_multitype_mark_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.input_sha256 != request.input_sha256
        || result.request_sha256 != request_sha
        || result.type_intercepts.len() != request.type_ids.len()
        || result.pair_potentials.len() != expected_potentials
        || result.pair_affinity_contrasts.len() != expected_affinities
        || result.posterior_predictive.observed_type_counts.len() != request.type_ids.len()
        || result.posterior_predictive.expected_type_counts.len() != request.type_ids.len()
        || result.posterior_predictive.mode != "one_step_conditionals_given_observed_neighbors"
        || !summaries_valid
        || !finite
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesError::WorkerContract(
            "conditional multitype backend result differs".into(),
        ));
    }
    Ok(())
}

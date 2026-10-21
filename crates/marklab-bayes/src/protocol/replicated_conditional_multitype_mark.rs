//! Patient-hierarchical conditional pseudolikelihood for hard multitype marks.
//!
//! Each pattern has its own `radius_um` graph (edges never cross patterns) and the
//! single-pattern model of `conditional_multitype_mark`. Its free intercept and pair-potential
//! vectors are `baseline + g * group_shift + patient_sd * z_patient + pattern_sd * z_pattern`
//! (non-centred, `z ~ Normal(0, 1)`), with `g = 1` for the comparison group and 0 for
//! `reference_group`; intercepts and potentials have separate patient and pattern SDs. Priors:
//! baselines `Normal(0, intercept_sd)` and `Normal(0, interaction_sd)`, group shifts
//! `Normal(0, group_effect_sd)`, patient SDs `HalfNormal(patient_sd_scale)`, pattern SDs
//! `HalfNormal(pattern_sd_scale)`. The reference-type gauge holds at every level. The estimand is
//! the population group shift in affinity contrasts (`group_affinity_shifts`, comparison minus
//! reference). The log-score comparison is in-sample against per-pattern independent labels;
//! `fit_state` uses the single-pattern NUTS policy and the claim does not depend on it.

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

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CsvPoint {
    pub pattern_id: String,
    pub patient_id: String,
    pub group: String,
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub type_id: String,
}

#[derive(Debug, Serialize)]
pub struct Patient {
    pub patient_id: String,
    pub group_index: usize,
}

#[derive(Debug, Serialize)]
pub struct Pattern {
    pub pattern_id: String,
    pub patient_index: usize,
    pub group_index: usize,
    pub point_count: usize,
    pub edge_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PointRow {
    pub point_id: String,
    pub pattern_index: usize,
    pub patient_index: usize,
    pub group_index: usize,
    pub type_index: usize,
    pub neighbor_counts: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct Edge {
    pub left: usize,
    pub right: usize,
    pub pattern_index: usize,
}

#[derive(Debug, Serialize)]
pub struct Priors {
    pub intercept_sd: f64,
    pub interaction_sd: f64,
    pub group_effect_sd: f64,
    pub patient_sd_scale: f64,
    pub pattern_sd_scale: f64,
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
    pub maximum_patients: usize,
    pub maximum_patterns: usize,
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
    pub group_ids: Vec<String>,
    pub reference_group: String,
    pub type_ids: Vec<String>,
    pub reference_type: String,
    pub radius_um: f64,
    pub patients: Vec<Patient>,
    pub patterns: Vec<Pattern>,
    pub points: Vec<PointRow>,
    pub edges: Vec<Edge>,
    pub priors: Priors,
    pub sampling: NutsSamplingSpec,
    pub diagnostic_policy: DiagnosticPolicy,
    pub resources: Resources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NamedScaleSummary {
    pub level: String,
    pub component: String,
    pub scale: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PairSummary {
    pub type_a: String,
    pub type_b: String,
    pub contrast: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatientPairSummary {
    pub patient_id: String,
    pub group: String,
    pub type_a: String,
    pub type_b: String,
    pub contrast: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatternCheck {
    pub pattern_id: String,
    pub observed_same_type_edges: u64,
    pub expected_same_type_edges: McseScalarSummary,
    pub observed_type_counts: Vec<u64>,
    pub expected_type_counts: Vec<McseScalarSummary>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub independent_pattern_label_composite_log_score: f64,
    pub spatial_hierarchical_composite_log_score: McseScalarSummary,
    pub mean_composite_log_score_improvement: f64,
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
    pub baseline_affinities: Vec<PairSummary>,
    pub group_affinity_shifts: Vec<PairSummary>,
    pub patient_affinities: Vec<PatientPairSummary>,
    pub hierarchy_scales: Vec<NamedScaleSummary>,
    pub comparison: Comparison,
    pub pattern_checks: Vec<PatternCheck>,
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
    pub group_ids: Vec<String>,
    pub reference_group: String,
    pub type_ids: Vec<String>,
    pub reference_type: String,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub point_count: usize,
    pub type_count: usize,
    pub edge_count: usize,
    pub baseline_affinities: Vec<PairSummary>,
    pub group_affinity_shifts: Vec<PairSummary>,
    pub patient_affinities: Vec<PatientPairSummary>,
    pub hierarchy_scales: Vec<NamedScaleSummary>,
    pub comparison: Comparison,
    pub pattern_checks: Vec<PatternCheck>,
    pub diagnostics: PosteriorDiagnostics,
    pub resources: Resources,
    pub statistical_unit: StatisticalUnit,
    pub null_model: NullHypothesis,
    pub assumptions: [String; 5],
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

impl Output {
    pub fn validate_for(
        &self,
        input_sha256: &str,
        reference_group: &str,
        reference_type: &str,
        radius_um: f64,
        backend: &BackendContract,
    ) -> Result<(), BayesError> {
        let expected_pairs = self.type_count * (self.type_count - 1) / 2;
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
        if self.format != "marklab.replicated_conditional_multitype_mark_fit"
            || self.version != 1
            || !self.backend.matches(backend)
            || self.input_sha256 != input_sha256
            || self.reference_group != reference_group
            || self.group_ids.first().map(String::as_str) != Some(reference_group)
            || self.reference_type != reference_type
            || self.type_ids.first().map(String::as_str) != Some(reference_type)
            || self.radius_um.to_bits() != radius_um.to_bits()
            || self.group_ids.len() != 2
            || self.patient_count < 4
            || self.pattern_count < self.patient_count * 2
            || self.point_count < 24
            || self.type_count != self.type_ids.len()
            || self.baseline_affinities.len() != expected_pairs
            || self.group_affinity_shifts.len() != expected_pairs
            || self.patient_affinities.len() != self.patient_count * expected_pairs
            || self.pattern_checks.len() != self.pattern_count
            || self.edge_count == 0
            || (self.fit_state == FitState::Complete) != complete
        {
            return Err(BayesError::WorkerContract(
                "durable replicated conditional-mark result differs".into(),
            ));
        }
        Ok(())
    }
}

pub fn validate_points(points: &[CsvPoint]) -> Result<(), BayesError> {
    let mut ids = HashSet::with_capacity(points.len());
    for point in points {
        if point.pattern_id.is_empty()
            || point.patient_id.is_empty()
            || point.group.is_empty()
            || point.point_id.is_empty()
            || point.type_id.is_empty()
            || point.pattern_id.len() > 256
            || point.patient_id.len() > 256
            || point.group.len() > 128
            || point.point_id.len() > 256
            || point.type_id.len() > 128
            || !point.x_um.is_finite()
            || !point.y_um.is_finite()
            || !ids.insert(point.point_id.as_str())
        {
            return Err(BayesError::InvalidSpec(
                "replicated conditional-mark point identity or value is invalid".into(),
            ));
        }
    }
    Ok(())
}

pub fn validate_result(
    result: &WorkerResult,
    request: &WorkerRequest,
    backend: &BackendContract,
    request_sha256: &str,
) -> Result<(), BayesError> {
    let expected_pairs = request.type_ids.len() * (request.type_ids.len() - 1) / 2;
    let summaries = result
        .baseline_affinities
        .iter()
        .map(|row| &row.contrast)
        .chain(result.group_affinity_shifts.iter().map(|row| &row.contrast))
        .chain(result.patient_affinities.iter().map(|row| &row.contrast))
        .chain(result.hierarchy_scales.iter().map(|row| &row.scale))
        .chain(result.pattern_checks.iter().flat_map(|row| {
            std::iter::once(&row.expected_same_type_edges).chain(row.expected_type_counts.iter())
        }))
        .chain(std::iter::once(
            &result.comparison.spatial_hierarchical_composite_log_score,
        ));
    let finite = summaries.into_iter().all(summary_finite)
        && result
            .pattern_checks
            .iter()
            .all(|row| row.expected_type_counts.len() == request.type_ids.len())
        && result
            .comparison
            .independent_pattern_label_composite_log_score
            .is_finite()
        && result
            .comparison
            .mean_composite_log_score_improvement
            .is_finite();
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
    if result.format != "marklab.pymc_replicated_conditional_multitype_mark_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.input_sha256 != request.input_sha256
        || result.request_sha256 != request_sha256
        || result.sampling.chains != request.sampling.chains
        || result.sampling.tune_per_chain != request.sampling.tune_per_chain
        || result.sampling.draws_per_chain != request.sampling.draws_per_chain
        || result.sampling.completed_draws
            != u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain)
        || result.baseline_affinities.len() != expected_pairs
        || result.group_affinity_shifts.len() != expected_pairs
        || result.patient_affinities.len() != request.patients.len() * expected_pairs
        || result.hierarchy_scales.len() != 4
        || result.pattern_checks.len() != request.patterns.len()
        || !finite
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesError::WorkerContract(
            "replicated conditional-mark backend result differs".into(),
        ));
    }
    Ok(())
}

fn summary_finite(summary: &McseScalarSummary) -> bool {
    summary.mean.is_finite()
        && summary.sd.is_finite()
        && summary.interval_lower.is_finite()
        && summary.interval_upper.is_finite()
        && summary.sd >= 0.0
        && summary.interval_lower <= summary.interval_upper
}

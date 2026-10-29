//! Patient-replicated multitype log-Gaussian Cox process on exact quadrature windows.
//!
//! For type `t`, node `n`, pattern `s` and patient `p`: `count ~ Poisson(weight_um2 *
//! exp(eta))` with `eta = intercept_t + group_effect_t * g_p + covariate_effect_t * x_n +
//! patient_effect_pt + pattern_effect_st + latent_tn`; node weights sum to the exact window area
//! and `g` is 0 for the reference group. Per-type priors: `intercept ~ Normal(intercept_mean,
//! intercept_sd)`, `group_effect ~ Normal(0, group_effect_sd)`, `covariate_effect ~ Normal(0,
//! covariate_effect_sd)`, `patient_sd ~ HalfNormal(patient_sd_scale)`, `pattern_sd ~
//! HalfNormal(pattern_sd_scale)`. Effects are non-centred; pattern effects sum to zero within
//! each patient and fields within each pattern. Each type has an independent field `L z` with
//! the fixed Matern-3/2 factor (marginal SD `field_amplitude`, length `field_length_scale_um`,
//! `jitter` added to the variance), block-diagonal by pattern. A fit is complete when draws are
//! finite, centring holds and the NUTS policy passes; it then claims
//! `experimental_replicated_multitype_point_process`, otherwise `diagnostic_only_nonconverged`.

use super::posterior_validation::{diagnostics_pass, summary_valid};
use crate::BackendContract;
use crate::BayesError;
use crate::DiagnosticPolicy;
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

#[derive(Debug, Serialize)]
pub struct Patient {
    pub patient_id: String,
    pub group: String,
    pub group_index: u8,
}

#[derive(Debug, Serialize)]
pub struct Pattern {
    pub pattern_id: String,
    pub patient_id: String,
    pub patient_index: usize,
    pub cohort: String,
    pub window_sha256: String,
    pub window_area_um2: f64,
    pub event_sha256: String,
    pub event_count: u64,
    pub node_count: usize,
}

#[derive(Debug, Serialize)]
pub struct Node {
    pub pattern_id: String,
    pub pattern_index: usize,
    pub patient_index: usize,
    pub node_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub weight_um2: f64,
    pub covariate: f64,
}

#[derive(Debug, Serialize)]
pub struct NodeTypeCount {
    pub node_index: usize,
    pub type_index: usize,
    pub count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Priors {
    pub intercept_mean: f64,
    pub intercept_sd: f64,
    pub group_effect_sd: f64,
    pub covariate_effect_sd: f64,
    pub patient_sd_scale: f64,
    pub pattern_sd_scale: f64,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub jitter: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub maximum_patients: usize,
    pub maximum_patterns: usize,
    pub maximum_types: usize,
    pub maximum_nodes_per_pattern: usize,
    pub maximum_total_nodes: usize,
    pub maximum_total_node_type_rows: usize,
    pub maximum_total_events: u64,
    pub maximum_draw_node_type_work: u64,
    pub draw_node_type_work: u64,
    pub timeout_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct WorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub input_sha256: String,
    pub reference_group: String,
    pub comparison_group: String,
    pub type_ids: Vec<String>,
    pub reference_type: String,
    pub patients: Vec<Patient>,
    pub patterns: Vec<Pattern>,
    pub nodes: Vec<Node>,
    pub node_type_counts: Vec<NodeTypeCount>,
    pub cholesky: Vec<f64>,
    pub priors: Priors,
    pub sampling: NutsSamplingSpec,
    pub diagnostic_policy: DiagnosticPolicy,
    pub resources: Resources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypePosterior {
    pub type_id: String,
    pub intercept: McseScalarSummary,
    pub group_effect: McseScalarSummary,
    pub covariate_effect: McseScalarSummary,
    pub patient_sd: McseScalarSummary,
    pub pattern_sd: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PairDifference {
    pub type_a: String,
    pub type_b: String,
    pub difference: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchyEffect {
    pub owner_id: String,
    pub type_id: String,
    pub effect: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NodeTypePosterior {
    pub pattern_id: String,
    pub node_id: String,
    pub type_id: String,
    pub latent_effect: McseScalarSummary,
    pub expected_count: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatternTypePredictive {
    pub pattern_id: String,
    pub type_id: String,
    pub observed_total_count: u64,
    pub replicate_count: u64,
    pub replicated_total_count_mean: f64,
    pub replicated_total_count_sd: f64,
    pub replicated_total_count_interval_lower: f64,
    pub replicated_total_count_interval_upper: f64,
    pub total_count_two_sided_tail_probability: f64,
    pub observed_node_count_variance: f64,
    pub replicated_node_count_variance_mean: f64,
    pub node_variance_two_sided_tail_probability: f64,
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
    pub type_posteriors: Vec<TypePosterior>,
    pub group_effect_differences: Vec<PairDifference>,
    pub patient_type_effects: Vec<HierarchyEffect>,
    pub pattern_type_effects: Vec<HierarchyEffect>,
    pub node_type_posteriors: Vec<NodeTypePosterior>,
    pub pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
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
    pub type_posteriors: Vec<TypePosterior>,
    pub group_effect_differences: Vec<PairDifference>,
    pub patient_type_effects: Vec<HierarchyEffect>,
    pub pattern_type_effects: Vec<HierarchyEffect>,
    pub node_type_posteriors: Vec<NodeTypePosterior>,
    pub pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
    pub diagnostics: PosteriorDiagnostics,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub type_count: usize,
    pub total_node_count: usize,
    pub total_node_type_row_count: usize,
    pub total_event_count: u64,
    pub groups: [String; 2],
    pub reference_type: String,
    pub cohort_count: usize,
    pub priors: Priors,
    pub resources: Resources,
    pub statistical_unit: StatisticalUnit,
    pub pattern_unit: String,
    pub likelihood: String,
    pub null_model: NullHypothesis,
    pub assumptions: [String; 7],
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

impl Output {
    pub fn validate_for(
        &self,
        request: &WorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        let policy = &request.diagnostic_policy;
        let diagnostics_pass = diagnostics_pass(&self.diagnostics, policy);
        let type_rows_valid = type_posteriors_valid(&self.type_posteriors, &request.type_ids);
        let pairs_valid = pair_differences_valid(&self.group_effect_differences, &request.type_ids);
        let hierarchy_valid = hierarchy_effects_valid(
            &self.patient_type_effects,
            request.patients.iter().map(|row| row.patient_id.as_str()),
            &request.type_ids,
        ) && hierarchy_effects_valid(
            &self.pattern_type_effects,
            request.patterns.iter().map(|row| row.pattern_id.as_str()),
            &request.type_ids,
        );
        let nodes_valid = self.node_type_posteriors.len() == request.node_type_counts.len()
            && self
                .node_type_posteriors
                .iter()
                .zip(&request.node_type_counts)
                .all(|(row, count)| {
                    let node = &request.nodes[count.node_index];
                    row.pattern_id == node.pattern_id
                        && row.node_id == node.node_id
                        && row.type_id == request.type_ids[count.type_index]
                        && summary_valid(&row.latent_effect)
                        && summary_valid(&row.expected_count)
                        && row.expected_count.mean > 0.0
                });
        if self.format != "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_fit"
            || self.version != 1
            || !self.backend.matches(&request.backend)
            || self.input_sha256 != request.input_sha256
            || self.request_sha256 != request_sha256
            || self.patient_count != request.patients.len()
            || self.pattern_count != request.patterns.len()
            || self.type_count != request.type_ids.len()
            || self.total_node_count != request.nodes.len()
            || self.total_node_type_row_count != request.node_type_counts.len()
            || self.total_event_count
                != request
                    .node_type_counts
                    .iter()
                    .map(|row| row.count)
                    .sum::<u64>()
            || self.groups
                != [
                    request.reference_group.clone(),
                    request.comparison_group.clone(),
                ]
            || self.reference_type != request.reference_type
            || self.priors != request.priors
            || self.resources != request.resources
            || self.sampling.completed_draws
                != u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain)
            || self.statistical_unit != StatisticalUnit::Patient
            || self.pattern_unit != "slide_pattern_nested_in_patient"
            || self.likelihood
                != "independent_type_specific_poisson_intensities_over_one_shared_exact_window"
            || !type_rows_valid
            || !pairs_valid
            || !hierarchy_valid
            || !nodes_valid
            || !predictive_valid(
                &self.pattern_type_posterior_predictive,
                request,
                self.sampling.completed_draws,
            )
            || (self.fit_state == FitState::Complete) != diagnostics_pass
        {
            return Err(BayesError::WorkerContract(
                "durable replicated multitype LGCP result differs".into(),
            ));
        }
        Ok(())
    }
}

pub fn validate_result(
    result: &WorkerResult,
    request: &WorkerRequest,
    request_sha256: &str,
) -> Result<(), BayesError> {
    let policy = &request.diagnostic_policy;
    let diagnostics_pass = diagnostics_pass(&result.diagnostics, policy);
    let types_valid = type_posteriors_valid(&result.type_posteriors, &request.type_ids);
    let pairs_valid = pair_differences_valid(&result.group_effect_differences, &request.type_ids);
    let hierarchy_valid = hierarchy_effects_valid(
        &result.patient_type_effects,
        request.patients.iter().map(|row| row.patient_id.as_str()),
        &request.type_ids,
    ) && hierarchy_effects_valid(
        &result.pattern_type_effects,
        request.patterns.iter().map(|row| row.pattern_id.as_str()),
        &request.type_ids,
    );
    let nodes_valid = result.node_type_posteriors.len() == request.node_type_counts.len()
        && result
            .node_type_posteriors
            .iter()
            .zip(&request.node_type_counts)
            .all(|(row, count)| {
                let node = &request.nodes[count.node_index];
                row.pattern_id == node.pattern_id
                    && row.node_id == node.node_id
                    && row.type_id == request.type_ids[count.type_index]
                    && summary_valid(&row.latent_effect)
                    && summary_valid(&row.expected_count)
                    && row.expected_count.mean > 0.0
            });
    let predictive_valid = predictive_valid(
        &result.pattern_type_posterior_predictive,
        request,
        result.sampling.completed_draws,
    );
    if result.format != "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_fit"
        || result.version != 1
        || !result.backend.matches(&request.backend)
        || result.input_sha256 != request.input_sha256
        || result.request_sha256 != request_sha256
        || result.sampling.completed_draws
            != u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain)
        || !types_valid
        || !pairs_valid
        || !hierarchy_valid
        || !nodes_valid
        || !predictive_valid
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesError::WorkerContract(
            "replicated multitype LGCP result differs".into(),
        ));
    }
    Ok(())
}

pub fn type_posteriors_valid(rows: &[TypePosterior], types: &[String]) -> bool {
    rows.len() == types.len()
        && rows.iter().zip(types).all(|(row, identity)| {
            row.type_id == *identity
                && [
                    &row.intercept,
                    &row.group_effect,
                    &row.covariate_effect,
                    &row.patient_sd,
                    &row.pattern_sd,
                ]
                .into_iter()
                .all(summary_valid)
        })
}

pub fn pair_differences_valid(rows: &[PairDifference], types: &[String]) -> bool {
    let expected = (0..types.len())
        .flat_map(|left| {
            (left + 1..types.len()).map(move |right| (types[left].as_str(), types[right].as_str()))
        })
        .collect::<Vec<_>>();
    rows.len() == expected.len()
        && rows.iter().zip(expected).all(|(row, (left, right))| {
            row.type_a == left && row.type_b == right && summary_valid(&row.difference)
        })
}

pub fn hierarchy_effects_valid<'a>(
    rows: &[HierarchyEffect],
    owners: impl Iterator<Item = &'a str>,
    types: &[String],
) -> bool {
    let expected = owners
        .flat_map(|owner| types.iter().map(move |identity| (owner, identity.as_str())))
        .collect::<Vec<_>>();
    rows.len() == expected.len()
        && rows.iter().zip(expected).all(|(row, (owner, identity))| {
            row.owner_id == owner && row.type_id == identity && summary_valid(&row.effect)
        })
}

fn predictive_valid(
    rows: &[PatternTypePredictive],
    request: &WorkerRequest,
    completed_draws: u64,
) -> bool {
    if rows.len() != request.patterns.len() * request.type_ids.len() {
        return false;
    }
    rows.iter().enumerate().all(|(index, row)| {
        let pattern_index = index / request.type_ids.len();
        let type_index = index % request.type_ids.len();
        let observed = request
            .node_type_counts
            .iter()
            .filter(|count| {
                request.nodes[count.node_index].pattern_index == pattern_index
                    && count.type_index == type_index
            })
            .map(|count| count.count)
            .sum::<u64>();
        let finite = [
            row.replicated_total_count_mean,
            row.replicated_total_count_sd,
            row.replicated_total_count_interval_lower,
            row.replicated_total_count_interval_upper,
            row.total_count_two_sided_tail_probability,
            row.observed_node_count_variance,
            row.replicated_node_count_variance_mean,
            row.node_variance_two_sided_tail_probability,
        ]
        .into_iter()
        .all(f64::is_finite);
        row.pattern_id == request.patterns[pattern_index].pattern_id
            && row.type_id == request.type_ids[type_index]
            && row.observed_total_count == observed
            && row.replicate_count == completed_draws
            && finite
            && row.replicated_total_count_sd >= 0.0
            && row.replicated_total_count_interval_lower
                <= row.replicated_total_count_interval_upper
            && row.observed_node_count_variance >= 0.0
            && row.replicated_node_count_variance_mean >= 0.0
            && (0.0..=1.0).contains(&row.total_count_two_sided_tail_probability)
            && (0.0..=1.0).contains(&row.node_variance_two_sided_tail_probability)
    })
}

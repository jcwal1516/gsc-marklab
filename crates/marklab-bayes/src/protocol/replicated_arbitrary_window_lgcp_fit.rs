//! Two-group patient hierarchy of fixed-kernel LGCPs over replicated exact-window patterns.
//!
//! Each pattern is one exact window of one patient, split into weighted quadrature nodes, with
//! `y ~ Poisson(w exp(eta))` and `eta = intercept + group_effect * g + covariate_effect * x +
//! patient_effect + pattern_effect + latent`; `g` is 0 for `reference_group` and 1 for
//! `comparison_group`, so `group_effect` is the comparison-minus-reference log-intensity shift.
//! Priors: `intercept ~ Normal(intercept_mean, intercept_sd)`, `group_effect ~ Normal(0,
//! group_effect_sd)`, `covariate_effect ~ Normal(0, covariate_effect_sd)`, `patient_sd ~
//! HalfNormal(patient_sd_scale)` and `pattern_sd ~ HalfNormal(pattern_sd_scale)`. Effects are
//! non-centred; pattern effects are centred within patient and `latent = L z` within pattern,
//! where `L` is block-diagonal by pattern: a Matern-3/2 factor with the fixed `field_amplitude`,
//! `field_length_scale_um` and `jitter` from `Priors` (constants, not priors). A complete fit
//! claims `experimental_replicated_pattern_hierarchy`; the null is `group_effect = 0`.

use super::posterior_validation::summary_valid;
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
    pub count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub maximum_patients: usize,
    pub maximum_patterns: usize,
    pub maximum_nodes_per_pattern: usize,
    pub maximum_total_nodes: usize,
    pub maximum_total_events: u64,
    pub maximum_draw_node_work: u64,
    pub draw_node_work: u64,
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
    pub patients: Vec<Patient>,
    pub patterns: Vec<Pattern>,
    pub nodes: Vec<Node>,
    pub cholesky: Vec<f64>,
    pub priors: Priors,
    pub sampling: NutsSamplingSpec,
    pub diagnostic_policy: DiagnosticPolicy,
    pub resources: Resources,
}

impl WorkerRequest {
    pub fn input_sha256(&self) -> &str {
        &self.input_sha256
    }

    pub fn first_node_identity(&self) -> (&str, &str) {
        (&self.nodes[0].pattern_id, &self.nodes[0].node_id)
    }

    pub fn completed_draws(&self) -> u64 {
        u64::from(self.sampling.chains) * u64::from(self.sampling.draws_per_chain)
    }

    pub fn patient_count(&self) -> usize {
        self.patients.len()
    }

    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn event_count(&self) -> u64 {
        self.nodes.iter().map(|node| node.count).sum()
    }

    pub fn cohort_count(&self) -> usize {
        self.patterns
            .iter()
            .map(|pattern| pattern.cohort.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }

    pub fn groups(&self) -> [String; 2] {
        [self.reference_group.clone(), self.comparison_group.clone()]
    }

    pub fn kernel_cube_work(&self) -> Option<u64> {
        self.patterns.iter().try_fold(0_u64, |work, pattern| {
            let nodes = pattern.node_count as u64;
            work.checked_add(nodes.checked_pow(3)?)
        })
    }

    pub fn diagnostic_policy(&self) -> &DiagnosticPolicy {
        &self.diagnostic_policy
    }

    pub fn patient_effects_valid(&self, rows: &[PatientEffect]) -> bool {
        rows.len() == self.patients.len()
            && rows.iter().zip(&self.patients).all(|(row, patient)| {
                row.patient_id == patient.patient_id && summary_valid(&row.effect)
            })
    }

    pub fn pattern_effects_valid(&self, rows: &[PatternEffect]) -> bool {
        rows.len() == self.patterns.len()
            && rows.iter().zip(&self.patterns).all(|(row, pattern)| {
                row.pattern_id == pattern.pattern_id && summary_valid(&row.effect)
            })
    }

    pub fn nodes_valid(&self, rows: &[NodePosterior]) -> bool {
        rows.len() == self.nodes.len()
            && rows.iter().zip(&self.nodes).all(|(row, node)| {
                row.pattern_id == node.pattern_id
                    && row.node_id == node.node_id
                    && summary_valid(&row.latent_effect)
                    && summary_valid(&row.expected_count)
                    && row.expected_count.mean > 0.0
            })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Posterior {
    pub intercept: McseScalarSummary,
    pub group_effect: McseScalarSummary,
    pub covariate_effect: McseScalarSummary,
    pub patient_sd: McseScalarSummary,
    pub pattern_sd: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatientEffect {
    pub patient_id: String,
    pub effect: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatternEffect {
    pub pattern_id: String,
    pub effect: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NodePosterior {
    pub pattern_id: String,
    pub node_id: String,
    pub latent_effect: McseScalarSummary,
    pub expected_count: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatternPosteriorPredictive {
    pub pattern_id: String,
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
    pub posterior: Posterior,
    pub patient_effects: Vec<PatientEffect>,
    pub pattern_effects: Vec<PatternEffect>,
    pub nodes: Vec<NodePosterior>,
    pub pattern_posterior_predictive: Vec<PatternPosteriorPredictive>,
    pub diagnostics: PosteriorDiagnostics,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultDocument {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub input_sha256: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: Posterior,
    pub patient_effects: Vec<PatientEffect>,
    pub pattern_effects: Vec<PatternEffect>,
    pub nodes: Vec<NodePosterior>,
    pub pattern_posterior_predictive: Vec<PatternPosteriorPredictive>,
    pub diagnostics: PosteriorDiagnostics,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub total_node_count: usize,
    pub total_event_count: u64,
    pub groups: [String; 2],
    pub cohort_count: usize,
    pub priors: Priors,
    pub resources: Resources,
    pub statistical_unit: StatisticalUnit,
    pub pattern_unit: String,
    pub null_model: NullHypothesis,
    pub assumptions: Vec<String>,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

pub fn validate_result(
    result: &WorkerResult,
    request: &WorkerRequest,
    request_sha256: &str,
) -> Result<(), BayesError> {
    let policy = &request.diagnostic_policy;
    let diagnostics_pass = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && result.diagnostics.divergences <= policy.maximum_divergences
        && result.diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits;
    let summaries = [
        &result.posterior.intercept,
        &result.posterior.group_effect,
        &result.posterior.covariate_effect,
        &result.posterior.patient_sd,
        &result.posterior.pattern_sd,
    ];
    let patient_effects_valid =
        result
            .patient_effects
            .iter()
            .zip(&request.patients)
            .all(|(effect, patient)| {
                effect.patient_id == patient.patient_id && summary_valid(&effect.effect)
            });
    let pattern_effects_valid =
        result
            .pattern_effects
            .iter()
            .zip(&request.patterns)
            .all(|(effect, pattern)| {
                effect.pattern_id == pattern.pattern_id && summary_valid(&effect.effect)
            });
    let predictive_valid = predictive_valid(
        &result.pattern_posterior_predictive,
        request,
        result.sampling.completed_draws,
    );
    let nodes_valid = result.nodes.len() == request.nodes.len()
        && result.nodes.iter().zip(&request.nodes).all(|(row, node)| {
            row.pattern_id == node.pattern_id
                && row.node_id == node.node_id
                && summary_valid(&row.latent_effect)
                && summary_valid(&row.expected_count)
                && row.expected_count.mean > 0.0
        });
    if result.format != "marklab.bayesian_replicated_arbitrary_window_lgcp_fit"
        || result.version != 2
        || !result.backend.matches(&request.backend)
        || result.input_sha256 != request.input_sha256
        || result.request_sha256 != request_sha256
        || result.sampling.completed_draws
            != u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain)
        || result.patient_effects.len() != request.patients.len()
        || result.pattern_effects.len() != request.patterns.len()
        || summaries.iter().any(|summary| !summary_valid(summary))
        || !patient_effects_valid
        || !pattern_effects_valid
        || !predictive_valid
        || !nodes_valid
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesError::WorkerContract(
            "replicated LGCP result differs".into(),
        ));
    }
    Ok(())
}

impl ResultDocument {
    pub fn validate_for(
        &self,
        request: &WorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        let policy = &request.diagnostic_policy;
        let diagnostics_pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
            && self.diagnostics.divergences <= policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits;
        let patient_effects_valid =
            self.patient_effects
                .iter()
                .zip(&request.patients)
                .all(|(effect, patient)| {
                    effect.patient_id == patient.patient_id && summary_valid(&effect.effect)
                });
        let pattern_effects_valid =
            self.pattern_effects
                .iter()
                .zip(&request.patterns)
                .all(|(effect, pattern)| {
                    effect.pattern_id == pattern.pattern_id && summary_valid(&effect.effect)
                });
        let predictive_valid = predictive_valid(
            &self.pattern_posterior_predictive,
            request,
            self.sampling.completed_draws,
        );
        let nodes_valid = self.nodes.len() == request.nodes.len()
            && self.nodes.iter().zip(&request.nodes).all(|(row, node)| {
                row.pattern_id == node.pattern_id
                    && row.node_id == node.node_id
                    && summary_valid(&row.latent_effect)
                    && summary_valid(&row.expected_count)
                    && row.expected_count.mean > 0.0
            });
        if self.format != "marklab.bayesian_replicated_arbitrary_window_lgcp_fit"
            || self.version != 2
            || !self.backend.matches(&request.backend)
            || self.input_sha256 != request.input_sha256
            || self.request_sha256 != request_sha256
            || self.patient_count != request.patients.len()
            || self.pattern_count != request.patterns.len()
            || self.total_node_count != request.nodes.len()
            || self.total_event_count != request.nodes.iter().map(|node| node.count).sum::<u64>()
            || self.patient_effects.len() != request.patients.len()
            || self.pattern_effects.len() != request.patterns.len()
            || !patient_effects_valid
            || !pattern_effects_valid
            || !predictive_valid
            || !nodes_valid
            || self.statistical_unit != StatisticalUnit::Patient
            || self.pattern_unit != "slide_pattern_nested_in_patient"
            || (self.fit_state == FitState::Complete) != diagnostics_pass
        {
            return Err(BayesError::WorkerContract(
                "replicated LGCP durable result identity or diagnostics differ".into(),
            ));
        }
        Ok(())
    }
}

pub fn predictive_valid(
    rows: &[PatternPosteriorPredictive],
    request: &WorkerRequest,
    completed_draws: u64,
) -> bool {
    rows.len() == request.patterns.len()
        && rows.iter().zip(&request.patterns).all(|(row, pattern)| {
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
            row.pattern_id == pattern.pattern_id
                && row.observed_total_count == pattern.event_count
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

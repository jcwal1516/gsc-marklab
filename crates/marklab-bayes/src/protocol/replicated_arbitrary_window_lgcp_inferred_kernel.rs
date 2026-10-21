//! Replicated patient LGCP whose Matern-3/2 amplitude and length scale are inferred and shared.
//!
//! The observation model, fixed effects, hierarchy priors and centring are those of
//! `replicated_arbitrary_window_lgcp_fit`, whose source request is embedded and must validate.
//! Each pattern gets an independent field `chol(K_p + jitter I) z_p`, centred within the
//! pattern, where `K_p` is Matern-3/2 over that pattern's node distances. One amplitude and one
//! length scale are shared by all patterns: `field_amplitude ~ HalfNormal(field_amplitude_scale)`
//! and `field_length_scale_um ~ HalfNormal(field_length_scale_scale_um)`; both scales and
//! `jitter` must equal the source request's fixed kernel values. The prior check covers positive
//! kernel draws and finite fields, not expected counts. A complete fit claims
//! `experimental_inferred_shared_kernel`, otherwise `diagnostic_only_nonconverged`.

use super::posterior_validation::summary_valid;
use super::replicated_arbitrary_window_lgcp_fit::predictive_valid;
use super::replicated_arbitrary_window_lgcp_fit::NodePosterior;
use super::replicated_arbitrary_window_lgcp_fit::PatientEffect;
use super::replicated_arbitrary_window_lgcp_fit::PatternEffect;
use super::replicated_arbitrary_window_lgcp_fit::PatternPosteriorPredictive;
use super::replicated_arbitrary_window_lgcp_fit::WorkerRequest as SourceRequest;
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::McseScalarSummary;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

pub struct ValidationContext<'a> {
    pub source_request: &'a SourceRequest,
    pub source_request_sha256: &'a str,
    pub backend: &'a BackendContract,
    pub request_sha256: &'a str,
    pub kernel_priors: &'a KernelPriors,
    pub resources: &'a KernelResources,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelPriors {
    pub field_amplitude_scale: f64,
    pub field_length_scale_scale_um: f64,
    pub jitter: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelResources {
    pub maximum_kernel_cube_work: u64,
    pub kernel_cube_work: u64,
    pub maximum_tree_depth: u32,
    pub timeout_seconds: u64,
}

#[derive(Serialize)]
pub struct WorkerRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub source_backend: BackendContract,
    pub source_request_sha256: &'a str,
    pub source_request: &'a SourceRequest,
    pub kernel_priors: KernelPriors,
    pub resources: KernelResources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Posterior {
    pub intercept: McseScalarSummary,
    pub group_effect: McseScalarSummary,
    pub covariate_effect: McseScalarSummary,
    pub patient_sd: McseScalarSummary,
    pub pattern_sd: McseScalarSummary,
    pub field_amplitude: McseScalarSummary,
    pub field_length_scale_um: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub source_backend: WorkerBackend,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: Posterior,
    pub patient_effects: Vec<PatientEffect>,
    pub pattern_effects: Vec<PatternEffect>,
    pub nodes: Vec<NodePosterior>,
    pub pattern_posterior_predictive: Vec<PatternPosteriorPredictive>,
    pub diagnostics: PosteriorDiagnostics,
    pub kernel_cube_work: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultDocument {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub source_backend: WorkerBackend,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
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
    pub cohort_count: usize,
    pub groups: [String; 2],
    pub kernel_priors: KernelPriors,
    pub resources: KernelResources,
    pub statistical_unit: StatisticalUnit,
    pub pattern_unit: String,
    pub null_model: NullHypothesis,
    pub assumptions: Vec<String>,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

pub fn validate_result(
    result: &WorkerResult,
    backend: &BackendContract,
    request_sha256: &str,
    source: &SourceRequest,
    source_request_sha256: &str,
    resources: &KernelResources,
) -> Result<(), BayesError> {
    let policy = source.diagnostic_policy();
    let summaries = [
        &result.posterior.intercept,
        &result.posterior.group_effect,
        &result.posterior.covariate_effect,
        &result.posterior.patient_sd,
        &result.posterior.pattern_sd,
        &result.posterior.field_amplitude,
        &result.posterior.field_length_scale_um,
    ];
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
    if result.format != "marklab.bayesian_replicated_arbitrary_window_lgcp_inferred_kernel_fit"
        || result.version != 1
        || !result.backend.matches(backend)
        || !result.source_backend.matches(&source.backend)
        || result.input_sha256 != source.input_sha256()
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_request_sha256
        || result.sampling.completed_draws != source.completed_draws()
        || result.kernel_cube_work != resources.kernel_cube_work
        || summaries.into_iter().any(|summary| !summary_valid(summary))
        || result.posterior.field_amplitude.mean <= 0.0
        || result.posterior.field_length_scale_um.mean <= 0.0
        || !source.patient_effects_valid(&result.patient_effects)
        || !source.pattern_effects_valid(&result.pattern_effects)
        || !source.nodes_valid(&result.nodes)
        || !predictive_valid(
            &result.pattern_posterior_predictive,
            source,
            result.sampling.completed_draws,
        )
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesError::WorkerContract(
            "replicated inferred-kernel LGCP result differs".into(),
        ));
    }
    Ok(())
}

impl ResultDocument {
    pub fn validate_for(&self, expected: &ValidationContext<'_>) -> Result<(), BayesError> {
        let source = expected.source_request;
        let policy = source.diagnostic_policy();
        let summaries = [
            &self.posterior.intercept,
            &self.posterior.group_effect,
            &self.posterior.covariate_effect,
            &self.posterior.patient_sd,
            &self.posterior.pattern_sd,
            &self.posterior.field_amplitude,
            &self.posterior.field_length_scale_um,
        ];
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
        let resources_match = self.resources.maximum_kernel_cube_work
            == expected.resources.maximum_kernel_cube_work
            && self.resources.kernel_cube_work == expected.resources.kernel_cube_work
            && self.resources.maximum_tree_depth == expected.resources.maximum_tree_depth
            && self.resources.timeout_seconds == expected.resources.timeout_seconds;
        let priors_match = self.kernel_priors.field_amplitude_scale
            == expected.kernel_priors.field_amplitude_scale
            && self.kernel_priors.field_length_scale_scale_um
                == expected.kernel_priors.field_length_scale_scale_um
            && self.kernel_priors.jitter == expected.kernel_priors.jitter;
        if self.format != "marklab.bayesian_replicated_arbitrary_window_lgcp_inferred_kernel_fit"
            || self.version != 1
            || !self.backend.matches(expected.backend)
            || !self.source_backend.matches(&source.backend)
            || self.input_sha256 != source.input_sha256()
            || self.request_sha256 != expected.request_sha256
            || self.source_request_sha256 != expected.source_request_sha256
            || self.sampling.completed_draws != source.completed_draws()
            || self.patient_count != source.patient_count()
            || self.pattern_count != source.pattern_count()
            || self.total_node_count != source.node_count()
            || self.total_event_count != source.event_count()
            || self.cohort_count != source.cohort_count()
            || self.groups != source.groups()
            || !resources_match
            || !priors_match
            || summaries.into_iter().any(|summary| !summary_valid(summary))
            || self.posterior.field_amplitude.mean <= 0.0
            || self.posterior.field_length_scale_um.mean <= 0.0
            || !source.patient_effects_valid(&self.patient_effects)
            || !source.pattern_effects_valid(&self.pattern_effects)
            || !source.nodes_valid(&self.nodes)
            || !predictive_valid(
                &self.pattern_posterior_predictive,
                source,
                self.sampling.completed_draws,
            )
            || self.statistical_unit != StatisticalUnit::Patient
            || self.pattern_unit != "slide_pattern_nested_in_patient"
            || (self.fit_state == FitState::Complete) != diagnostics_pass
        {
            return Err(BayesError::WorkerContract(
                "durable inferred-kernel LGCP result identity or diagnostics differ".into(),
            ));
        }
        Ok(())
    }
}

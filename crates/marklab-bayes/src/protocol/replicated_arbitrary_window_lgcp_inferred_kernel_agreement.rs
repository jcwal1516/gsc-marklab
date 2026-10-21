//! Typed messages and scientific checks for replicated arbitrary window lgcp inferred kernel agreement.

use super::posterior_validation::summary_valid;
use super::replicated_arbitrary_window_lgcp_agreement::compare_scalar;
use super::replicated_arbitrary_window_lgcp_agreement::compare_vector;
use super::replicated_arbitrary_window_lgcp_agreement::ScalarAgreement;
use super::replicated_arbitrary_window_lgcp_agreement::VectorAgreement;
use super::replicated_arbitrary_window_lgcp_fit::NodePosterior;
use super::replicated_arbitrary_window_lgcp_fit::PatientEffect;
use super::replicated_arbitrary_window_lgcp_fit::PatternEffect;
use super::replicated_arbitrary_window_lgcp_fit::WorkerRequest as SourceRequest;
use super::replicated_arbitrary_window_lgcp_inferred_kernel::Posterior;
use super::replicated_arbitrary_window_lgcp_inferred_kernel::ResultDocument;
use crate::backend_versions::JAX_VERSION;
use crate::AgreementStatus;
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub input_sha256: String,
    pub maximum_tree_depth: u32,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: Posterior,
    pub patient_effects: Vec<PatientEffect>,
    pub pattern_effects: Vec<PatternEffect>,
    pub nodes: Vec<NodePosterior>,
    pub diagnostics: PosteriorDiagnostics,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub intercept: ScalarAgreement,
    pub group_effect: ScalarAgreement,
    pub covariate_effect: ScalarAgreement,
    pub patient_sd: ScalarAgreement,
    pub pattern_sd: ScalarAgreement,
    pub field_amplitude: ScalarAgreement,
    pub field_length_scale_um: ScalarAgreement,
    pub patient_effects: VectorAgreement,
    pub pattern_effects: VectorAgreement,
    pub latent_effect: VectorAgreement,
    pub expected_count: VectorAgreement,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub node_count: usize,
    pub all_pass: bool,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub format: &'static str,
    pub version: u32,
    pub pymc: ResultDocument,
    pub numpyro: NumpyroResult,
    pub comparison: Comparison,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub statistical_unit: StatisticalUnit,
    pub assumptions: [&'static str; 3],
    pub finite_result_policy: &'static str,
    pub claim_status: ClaimStatus,
}

pub fn validate_numpyro(
    result: &NumpyroResult,
    source: &SourceRequest,
    source_request_sha256: &str,
    request_sha: &str,
    backend: &BackendContract,
    depth: u32,
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
    let complete = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && result.diagnostics.divergences == 0
        && result.diagnostics.max_tree_depth_hits == 0;
    if result.format != "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_request_sha256
        || result.input_sha256 != source.input_sha256()
        || result.maximum_tree_depth != depth
        || result.sampling.completed_draws != source.completed_draws()
        || summaries.into_iter().any(|value| !summary_valid(value))
        || !source.patient_effects_valid(&result.patient_effects)
        || !source.pattern_effects_valid(&result.pattern_effects)
        || !source.nodes_valid(&result.nodes)
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesError::WorkerContract(
            "NumPyro inferred-kernel result differs".into(),
        ));
    }
    Ok(())
}

pub fn compare(
    left: &ResultDocument,
    right: &NumpyroResult,
    maximum: f64,
    parameter_tolerance: f64,
    field_tolerance: f64,
) -> Comparison {
    let scalar = |a, b| compare_scalar(a, b, maximum, parameter_tolerance);
    let intercept = scalar(&left.posterior.intercept, &right.posterior.intercept);
    let group_effect = scalar(&left.posterior.group_effect, &right.posterior.group_effect);
    let covariate_effect = scalar(
        &left.posterior.covariate_effect,
        &right.posterior.covariate_effect,
    );
    let patient_sd = scalar(&left.posterior.patient_sd, &right.posterior.patient_sd);
    let pattern_sd = scalar(&left.posterior.pattern_sd, &right.posterior.pattern_sd);
    let field_amplitude = scalar(
        &left.posterior.field_amplitude,
        &right.posterior.field_amplitude,
    );
    let field_length_scale_um = scalar(
        &left.posterior.field_length_scale_um,
        &right.posterior.field_length_scale_um,
    );
    let vector = |a: Vec<_>, b: Vec<_>| {
        compare_vector(a.into_iter(), b.into_iter(), maximum, field_tolerance)
    };
    let patient_effects = vector(
        left.patient_effects.iter().map(|row| &row.effect).collect(),
        right
            .patient_effects
            .iter()
            .map(|row| &row.effect)
            .collect(),
    );
    let pattern_effects = vector(
        left.pattern_effects.iter().map(|row| &row.effect).collect(),
        right
            .pattern_effects
            .iter()
            .map(|row| &row.effect)
            .collect(),
    );
    let latent_effect = vector(
        left.nodes.iter().map(|row| &row.latent_effect).collect(),
        right.nodes.iter().map(|row| &row.latent_effect).collect(),
    );
    let expected_count = vector(
        left.nodes.iter().map(|row| &row.expected_count).collect(),
        right.nodes.iter().map(|row| &row.expected_count).collect(),
    );
    let all_pass = [
        intercept.passes,
        group_effect.passes,
        covariate_effect.passes,
        patient_sd.passes,
        pattern_sd.passes,
        field_amplitude.passes,
        field_length_scale_um.passes,
        patient_effects.passes,
        pattern_effects.passes,
        latent_effect.passes,
        expected_count.passes,
    ]
    .into_iter()
    .all(|value| value);
    Comparison {
        intercept,
        group_effect,
        covariate_effect,
        patient_sd,
        pattern_sd,
        field_amplitude,
        field_length_scale_um,
        patient_effects,
        pattern_effects,
        latent_effect,
        expected_count,
        patient_count: left.patient_effects.len(),
        pattern_count: left.pattern_effects.len(),
        node_count: left.nodes.len(),
        all_pass,
    }
}

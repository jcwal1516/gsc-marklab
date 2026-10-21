//! Typed messages and scientific checks for replicated arbitrary window lgcp agreement.

use super::posterior_validation::summary_valid;
use super::replicated_arbitrary_window_lgcp_fit::predictive_valid;
use super::replicated_arbitrary_window_lgcp_fit::NodePosterior;
use super::replicated_arbitrary_window_lgcp_fit::PatientEffect;
use super::replicated_arbitrary_window_lgcp_fit::PatternEffect;
use super::replicated_arbitrary_window_lgcp_fit::PatternPosteriorPredictive;
use super::replicated_arbitrary_window_lgcp_fit::Posterior;
use super::replicated_arbitrary_window_lgcp_fit::ResultDocument;
use super::replicated_arbitrary_window_lgcp_fit::WorkerRequest;
use crate::backend_versions::JAX_VERSION;
use crate::combined_mcse;
use crate::intervals_overlap;
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

#[derive(Serialize)]
pub struct NumpyroRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: &'a str,
    pub source_request: &'a WorkerRequest,
    pub maximum_tree_depth: u32,
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
    pub posterior: Posterior,
    pub patient_effects: Vec<PatientEffect>,
    pub pattern_effects: Vec<PatternEffect>,
    pub nodes: Vec<NodePosterior>,
    pub pattern_posterior_predictive: Vec<PatternPosteriorPredictive>,
    pub diagnostics: PosteriorDiagnostics,
}

#[derive(Debug, Serialize)]
pub struct ScalarAgreement {
    pub absolute_mean_difference: f64,
    pub combined_mcse: f64,
    pub standardized_difference: f64,
    pub intervals_overlap: bool,
    pub passes: bool,
}

#[derive(Debug, Serialize)]
pub struct VectorAgreement {
    pub element_count: usize,
    pub maximum_standardized_difference: f64,
    pub all_intervals_overlap: bool,
    pub passes: bool,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub intercept: ScalarAgreement,
    pub group_effect: ScalarAgreement,
    pub covariate_effect: ScalarAgreement,
    pub patient_sd: ScalarAgreement,
    pub pattern_sd: ScalarAgreement,
    pub patient_effects: VectorAgreement,
    pub pattern_effects: VectorAgreement,
    pub latent_effect: VectorAgreement,
    pub expected_count: VectorAgreement,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub node_count: usize,
    pub maximum_standardized_difference: f64,
    pub minimum_parameter_tolerance: f64,
    pub minimum_field_tolerance: f64,
    pub all_pass: bool,
}

#[derive(Debug, Serialize)]
pub struct AgreementResult {
    pub format: String,
    pub version: u32,
    pub pymc: ResultDocument,
    pub numpyro: NumpyroResult,
    pub comparison: Comparison,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub statistical_unit: StatisticalUnit,
    pub assumptions: Vec<String>,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

pub fn validate_numpyro(
    result: &NumpyroResult,
    source: &WorkerRequest,
    source_sha256: &str,
    maximum_tree_depth: u32,
    pymc: &ResultDocument,
    backend: &BackendContract,
    request_sha256: &str,
) -> Result<(), BayesError> {
    let summaries = [
        &result.posterior.intercept,
        &result.posterior.group_effect,
        &result.posterior.covariate_effect,
        &result.posterior.patient_sd,
        &result.posterior.pattern_sd,
    ];
    let identities_match = result
        .patient_effects
        .iter()
        .zip(&pymc.patient_effects)
        .all(|(left, right)| left.patient_id == right.patient_id && summary_valid(&left.effect))
        && result
            .pattern_effects
            .iter()
            .zip(&pymc.pattern_effects)
            .all(|(left, right)| {
                left.pattern_id == right.pattern_id && summary_valid(&left.effect)
            })
        && result.nodes.iter().zip(&pymc.nodes).all(|(left, right)| {
            left.pattern_id == right.pattern_id
                && left.node_id == right.node_id
                && summary_valid(&left.latent_effect)
                && summary_valid(&left.expected_count)
                && left.expected_count.mean > 0.0
        });
    let diagnostics_pass = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= 1.01
        && result.diagnostics.ess_bulk >= 400.0
        && result.diagnostics.ess_tail >= 400.0
        && result.diagnostics.minimum_ebfmi >= 0.2
        && result.diagnostics.divergences == 0
        && result.diagnostics.max_tree_depth_hits == 0;
    if result.format != "marklab.numpyro_replicated_arbitrary_window_lgcp_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != pymc.input_sha256
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_sha256
        || result.maximum_tree_depth != maximum_tree_depth
        || result.sampling.completed_draws != pymc.sampling.completed_draws
        || result.patient_effects.len() != pymc.patient_effects.len()
        || result.pattern_effects.len() != pymc.pattern_effects.len()
        || result.nodes.len() != pymc.nodes.len()
        || result.pattern_posterior_predictive.len() != pymc.pattern_posterior_predictive.len()
        || !predictive_valid(
            &result.pattern_posterior_predictive,
            source,
            result.sampling.completed_draws,
        )
        || summaries.into_iter().any(|summary| !summary_valid(summary))
        || !identities_match
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesError::WorkerContract(
            "NumPyro replicated LGCP result differs".into(),
        ));
    }
    Ok(())
}

pub fn compare(
    pymc: &ResultDocument,
    numpyro: &NumpyroResult,
    maximum: f64,
    parameter_tolerance: f64,
    field_tolerance: f64,
) -> Comparison {
    let intercept = compare_scalar(
        &pymc.posterior.intercept,
        &numpyro.posterior.intercept,
        maximum,
        parameter_tolerance,
    );
    let group_effect = compare_scalar(
        &pymc.posterior.group_effect,
        &numpyro.posterior.group_effect,
        maximum,
        parameter_tolerance,
    );
    let covariate_effect = compare_scalar(
        &pymc.posterior.covariate_effect,
        &numpyro.posterior.covariate_effect,
        maximum,
        parameter_tolerance,
    );
    let patient_sd = compare_scalar(
        &pymc.posterior.patient_sd,
        &numpyro.posterior.patient_sd,
        maximum,
        parameter_tolerance,
    );
    let pattern_sd = compare_scalar(
        &pymc.posterior.pattern_sd,
        &numpyro.posterior.pattern_sd,
        maximum,
        parameter_tolerance,
    );
    let patient_effects = compare_vector(
        pymc.patient_effects.iter().map(|value| &value.effect),
        numpyro.patient_effects.iter().map(|value| &value.effect),
        maximum,
        field_tolerance,
    );
    let pattern_effects = compare_vector(
        pymc.pattern_effects.iter().map(|value| &value.effect),
        numpyro.pattern_effects.iter().map(|value| &value.effect),
        maximum,
        field_tolerance,
    );
    let latent_effect = compare_vector(
        pymc.nodes.iter().map(|value| &value.latent_effect),
        numpyro.nodes.iter().map(|value| &value.latent_effect),
        maximum,
        field_tolerance,
    );
    let expected_count = compare_vector(
        pymc.nodes.iter().map(|value| &value.expected_count),
        numpyro.nodes.iter().map(|value| &value.expected_count),
        maximum,
        field_tolerance,
    );
    let all_pass = [
        intercept.passes,
        group_effect.passes,
        covariate_effect.passes,
        patient_sd.passes,
        pattern_sd.passes,
        patient_effects.passes,
        pattern_effects.passes,
        latent_effect.passes,
        expected_count.passes,
    ]
    .into_iter()
    .all(|passes| passes);
    Comparison {
        intercept,
        group_effect,
        covariate_effect,
        patient_sd,
        pattern_sd,
        patient_effects,
        pattern_effects,
        latent_effect,
        expected_count,
        patient_count: pymc.patient_effects.len(),
        pattern_count: pymc.pattern_effects.len(),
        node_count: pymc.nodes.len(),
        maximum_standardized_difference: maximum,
        minimum_parameter_tolerance: parameter_tolerance,
        minimum_field_tolerance: field_tolerance,
        all_pass,
    }
}

pub fn compare_scalar(
    left: &McseScalarSummary,
    right: &McseScalarSummary,
    maximum: f64,
    tolerance: f64,
) -> ScalarAgreement {
    let difference = (left.mean - right.mean).abs();
    let combined_mcse = combined_mcse(left, right);
    let standardized = difference / combined_mcse.max(tolerance);
    let intervals_overlap = intervals_overlap(left, right);
    ScalarAgreement {
        absolute_mean_difference: difference,
        combined_mcse,
        standardized_difference: standardized,
        intervals_overlap,
        passes: standardized <= maximum && intervals_overlap,
    }
}

pub fn compare_vector<'a>(
    left: impl Iterator<Item = &'a McseScalarSummary>,
    right: impl Iterator<Item = &'a McseScalarSummary>,
    maximum: f64,
    tolerance: f64,
) -> VectorAgreement {
    let comparisons = left
        .zip(right)
        .map(|(left, right)| compare_scalar(left, right, maximum, tolerance))
        .collect::<Vec<_>>();
    let maximum_standardized_difference = comparisons
        .iter()
        .map(|value| value.standardized_difference)
        .fold(0.0_f64, f64::max);
    let all_intervals_overlap = comparisons.iter().all(|value| value.intervals_overlap);
    VectorAgreement {
        element_count: comparisons.len(),
        maximum_standardized_difference,
        all_intervals_overlap,
        passes: maximum_standardized_difference <= maximum && all_intervals_overlap,
    }
}

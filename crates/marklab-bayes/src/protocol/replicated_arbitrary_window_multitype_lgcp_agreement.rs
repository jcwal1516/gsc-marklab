//! Typed messages and scientific checks for replicated arbitrary window multitype lgcp agreement.

use super::posterior_validation::diagnostics_pass;
use super::replicated_arbitrary_window_lgcp_agreement::compare_vector;
use super::replicated_arbitrary_window_lgcp_agreement::VectorAgreement;
use super::replicated_arbitrary_window_multitype_lgcp::Output as FitOutput;
use crate::backend_versions::JAX_VERSION;
use crate::AgreementStatus;
use crate::BayesError;
use crate::DiagnosticPolicy;
use crate::FitState;
use crate::McseScalarSummary;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
pub struct TypeRow {
    pub type_id: String,
    pub intercept: McseScalarSummary,
    pub group_effect: McseScalarSummary,
    pub covariate_effect: McseScalarSummary,
    pub patient_sd: McseScalarSummary,
    pub pattern_sd: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct DifferenceRow {
    pub type_a: String,
    pub type_b: String,
    pub difference: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct EffectRow {
    pub owner_id: String,
    pub type_id: String,
    pub effect: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct NodeRow {
    pub pattern_id: String,
    pub node_id: String,
    pub type_id: String,
    pub latent_effect: McseScalarSummary,
    pub expected_count: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct FitView {
    pub input_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub type_posteriors: Vec<TypeRow>,
    pub group_effect_differences: Vec<DifferenceRow>,
    pub patient_type_effects: Vec<EffectRow>,
    pub pattern_type_effects: Vec<EffectRow>,
    pub node_type_posteriors: Vec<NodeRow>,
}

#[derive(Debug, Deserialize)]
pub struct NumpyroView {
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
    pub type_posteriors: Vec<TypeRow>,
    pub group_effect_differences: Vec<DifferenceRow>,
    pub patient_type_effects: Vec<EffectRow>,
    pub pattern_type_effects: Vec<EffectRow>,
    pub node_type_posteriors: Vec<NodeRow>,
    pub diagnostics: PosteriorDiagnostics,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub type_parameters: VectorAgreement,
    pub group_effect_differences: VectorAgreement,
    pub patient_type_effects: VectorAgreement,
    pub pattern_type_effects: VectorAgreement,
    pub latent_effects: VectorAgreement,
    pub expected_counts: VectorAgreement,
    pub maximum_standardized_difference: f64,
    pub minimum_parameter_tolerance: f64,
    pub minimum_field_tolerance: f64,
    pub all_pass: bool,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub format: &'static str,
    pub version: u32,
    pub pymc: FitOutput,
    pub numpyro: serde_json::Value,
    pub comparison: Comparison,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub statistical_unit: StatisticalUnit,
    pub assumptions: [&'static str; 4],
    pub finite_result_policy: &'static str,
    pub claim_status: ClaimStatus,
}

pub fn validate_numpyro(
    result: &NumpyroView,
    baseline: &FitView,
    backend: &crate::BackendContract,
    request_sha: &str,
    source_sha: &str,
    depth: u32,
) -> Result<(), BayesError> {
    let policy = DiagnosticPolicy::default();
    let diagnostic_pass = diagnostics_pass(&result.diagnostics, &policy);
    let identities_match = rows_match(baseline, result);
    if result.format != "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_result"
        || result.version != 1
        || !result.backend.matches(backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != baseline.input_sha256
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_sha
        || result.maximum_tree_depth != depth
        || result.sampling.completed_draws != baseline.sampling.completed_draws
        || !identities_match
        || (result.fit_state == FitState::Complete) != diagnostic_pass
    {
        return Err(BayesError::WorkerContract(
            "NumPyro replicated multitype LGCP result differs".into(),
        ));
    }
    Ok(())
}

fn rows_match(left: &FitView, right: &NumpyroView) -> bool {
    left.type_posteriors.len() == right.type_posteriors.len()
        && left
            .type_posteriors
            .iter()
            .zip(&right.type_posteriors)
            .all(|(a, b)| a.type_id == b.type_id)
        && left.group_effect_differences.len() == right.group_effect_differences.len()
        && left
            .group_effect_differences
            .iter()
            .zip(&right.group_effect_differences)
            .all(|(a, b)| a.type_a == b.type_a && a.type_b == b.type_b)
        && effect_rows_match(&left.patient_type_effects, &right.patient_type_effects)
        && effect_rows_match(&left.pattern_type_effects, &right.pattern_type_effects)
        && left.node_type_posteriors.len() == right.node_type_posteriors.len()
        && left
            .node_type_posteriors
            .iter()
            .zip(&right.node_type_posteriors)
            .all(|(a, b)| {
                a.pattern_id == b.pattern_id && a.node_id == b.node_id && a.type_id == b.type_id
            })
}

fn effect_rows_match(left: &[EffectRow], right: &[EffectRow]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.owner_id == b.owner_id && a.type_id == b.type_id)
}

pub fn compare(
    pymc: &FitView,
    numpyro: &NumpyroView,
    maximum: f64,
    parameter_tolerance: f64,
    field_tolerance: f64,
) -> Comparison {
    let type_parameters = compare_vector(
        pymc.type_posteriors.iter().flat_map(type_summaries),
        numpyro.type_posteriors.iter().flat_map(type_summaries),
        maximum,
        parameter_tolerance,
    );
    let group_effect_differences = compare_vector(
        pymc.group_effect_differences
            .iter()
            .map(|row| &row.difference),
        numpyro
            .group_effect_differences
            .iter()
            .map(|row| &row.difference),
        maximum,
        parameter_tolerance,
    );
    let patient_type_effects = compare_vector(
        pymc.patient_type_effects.iter().map(|row| &row.effect),
        numpyro.patient_type_effects.iter().map(|row| &row.effect),
        maximum,
        parameter_tolerance,
    );
    let pattern_type_effects = compare_vector(
        pymc.pattern_type_effects.iter().map(|row| &row.effect),
        numpyro.pattern_type_effects.iter().map(|row| &row.effect),
        maximum,
        parameter_tolerance,
    );
    let latent_effects = compare_vector(
        pymc.node_type_posteriors
            .iter()
            .map(|row| &row.latent_effect),
        numpyro
            .node_type_posteriors
            .iter()
            .map(|row| &row.latent_effect),
        maximum,
        field_tolerance,
    );
    let expected_counts = compare_vector(
        pymc.node_type_posteriors
            .iter()
            .map(|row| &row.expected_count),
        numpyro
            .node_type_posteriors
            .iter()
            .map(|row| &row.expected_count),
        maximum,
        field_tolerance,
    );
    let all_pass = [
        &type_parameters,
        &group_effect_differences,
        &patient_type_effects,
        &pattern_type_effects,
        &latent_effects,
        &expected_counts,
    ]
    .into_iter()
    .all(|row| row.passes);
    Comparison {
        type_parameters,
        group_effect_differences,
        patient_type_effects,
        pattern_type_effects,
        latent_effects,
        expected_counts,
        maximum_standardized_difference: maximum,
        minimum_parameter_tolerance: parameter_tolerance,
        minimum_field_tolerance: field_tolerance,
        all_pass,
    }
}

fn type_summaries(row: &TypeRow) -> [&McseScalarSummary; 5] {
    [
        &row.intercept,
        &row.group_effect,
        &row.covariate_effect,
        &row.patient_sd,
        &row.pattern_sd,
    ]
}

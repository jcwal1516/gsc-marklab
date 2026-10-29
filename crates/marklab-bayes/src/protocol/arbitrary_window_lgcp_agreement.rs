//! Typed messages and scientific checks for arbitrary window lgcp agreement.

use super::arbitrary_window_lgcp_fit;
use super::arbitrary_window_lgcp_fit::FitResult;
use super::arbitrary_window_lgcp_fit::InputIdentity;
use super::arbitrary_window_lgcp_fit::ResultNode;
use super::posterior_validation::{diagnostics_pass, scalar_valid};
use crate::backend_versions::JAX_VERSION;
use crate::compare_elements;
use crate::compare_parameter;
use crate::AgreementStatus;
use crate::BackendContract;
use crate::BayesError;
use crate::ElementwiseAgreement;
use crate::FitState;
use crate::GriddedLgcpPosterior;
use crate::GriddedLgcpPosteriorPredictive;
use crate::McseScalarSummary;
use crate::ParameterAgreement;
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
    pub source_backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: &'a str,
    pub source_request: &'a arbitrary_window_lgcp_fit::WorkerRequest,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroNode {
    node_id: String,
    pub latent_effect: McseScalarSummary,
    pub expected_count: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    source_backend: WorkerBackend,
    jax_version: String,
    input: InputIdentity,
    request_sha256: String,
    source_request_sha256: String,
    covariance_sha256: String,
    pub fit_state: FitState,
    sampling: SamplingSummary,
    posterior: GriddedLgcpPosterior,
    nodes: Vec<NumpyroNode>,
    diagnostics: PosteriorDiagnostics,
    posterior_predictive: GriddedLgcpPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub intercept: ParameterAgreement,
    pub coefficient: ParameterAgreement,
    pub latent_effect: ElementwiseAgreement,
    pub expected_count: ElementwiseAgreement,
    node_count: usize,
    maximum_standardized_difference: f64,
    minimum_parameter_tolerance: f64,
    minimum_field_tolerance: f64,
}

#[derive(Debug, Serialize)]
pub struct AgreementResult {
    pub format: String,
    pub version: u32,
    pub input: InputIdentity,
    pub covariance_sha256: String,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub pymc: FitResult,
    pub numpyro: NumpyroResult,
    pub comparison: Comparison,
    pub statistical_unit: StatisticalUnit,
    pub claim_status: ClaimStatus,
}

pub fn validate_numpyro(
    result: &NumpyroResult,
    request: &NumpyroRequest<'_>,
    request_sha256: &str,
    pymc: &FitResult,
) -> Result<(), BayesError> {
    let policy = &request.source_request.source_request.diagnostic_policy;
    let diagnostics_pass = diagnostics_pass(&result.diagnostics, policy);
    let nodes_valid = result.nodes.len() == pymc.nodes.len()
        && result
            .nodes
            .iter()
            .zip(&pymc.nodes)
            .all(|(result, source)| {
                result.node_id == source.node_id
                    && scalar_valid(&result.latent_effect, false)
                    && scalar_valid(&result.expected_count, true)
            });
    let draws = u64::from(request.source_request.source_request.sampling.chains)
        * u64::from(
            request
                .source_request
                .source_request
                .sampling
                .draws_per_chain,
        );
    let predictive = &result.posterior_predictive;
    if result.format != "marklab.numpyro_arbitrary_window_lgcp_result"
        || result.version != 1
        || !result.backend.matches(&request.backend)
        || !result.source_backend.matches(&request.source_backend)
        || result.jax_version != JAX_VERSION
        || result.input != pymc.input
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != request.source_request_sha256
        || result.covariance_sha256 != request.source_request.covariance_sha256
        || result.sampling.chains != request.source_request.source_request.sampling.chains
        || result.sampling.tune_per_chain
            != request
                .source_request
                .source_request
                .sampling
                .tune_per_chain
        || result.sampling.draws_per_chain
            != request
                .source_request
                .source_request
                .sampling
                .draws_per_chain
        || result.sampling.completed_draws != draws
        || !scalar_valid(&result.posterior.intercept, false)
        || !scalar_valid(&result.posterior.coefficient, false)
        || !nodes_valid
        || predictive.observed_total_count
            != request
                .source_request
                .nodes
                .iter()
                .map(|node| node.count)
                .sum::<u64>()
        || ![
            predictive.replicated_total_mean,
            predictive.replicated_total_sd,
            predictive.replicated_zero_cells_mean,
        ]
        .into_iter()
        .all(f64::is_finite)
        || predictive.replicated_total_mean < 0.0
        || predictive.replicated_total_sd < 0.0
        || predictive.replicated_zero_cells_mean < 0.0
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesError::WorkerContract(
            "NumPyro arbitrary-window LGCP result identity or diagnostics differ".into(),
        ));
    }
    Ok(())
}

pub fn compare(
    pymc: &FitResult,
    numpyro: &NumpyroResult,
    maximum_standardized_difference: f64,
    minimum_parameter_tolerance: f64,
    minimum_field_tolerance: f64,
) -> Comparison {
    let field = |select: fn(&ResultNode) -> McseScalarSummary,
                 numpyro_select: fn(&NumpyroNode) -> McseScalarSummary| {
        compare_elements(
            pymc.nodes
                .iter()
                .map(select)
                .zip(numpyro.nodes.iter().map(numpyro_select)),
            maximum_standardized_difference,
            minimum_field_tolerance,
        )
    };
    Comparison {
        intercept: compare_parameter(
            &pymc.posterior.intercept,
            &numpyro.posterior.intercept,
            maximum_standardized_difference,
            minimum_parameter_tolerance,
        ),
        coefficient: compare_parameter(
            &pymc.posterior.coefficient,
            &numpyro.posterior.coefficient,
            maximum_standardized_difference,
            minimum_parameter_tolerance,
        ),
        latent_effect: field(|node| node.latent_effect, |node| node.latent_effect),
        expected_count: field(|node| node.expected_count, |node| node.expected_count),
        node_count: pymc.nodes.len(),
        maximum_standardized_difference,
        minimum_parameter_tolerance,
        minimum_field_tolerance,
    }
}

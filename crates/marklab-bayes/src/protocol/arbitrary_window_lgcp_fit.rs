//! Log-Gaussian Cox process fitted to one point pattern in an exact window, on weighted nodes.
//!
//! Events are assigned to 4 to 36 positive-area quadrature nodes, and node counts follow
//! `y_k ~ Poisson(w_k exp(intercept + coefficient * x_k + offset_k + u_k))`: a binned
//! approximation that holds the intensity constant within each node region. The field is
//! `u = L z`, `z ~ Normal(0, I)`, where `L` is the Cholesky factor of a fixed Matern-3/2
//! covariance `field_amplitude^2 (1 + sqrt(3) d / l) exp(-sqrt(3) d / l) + jitter I` between node
//! representatives with `l = field_length_scale_um`; amplitude, length scale and jitter are
//! inputs, not inferred. Priors are `intercept ~ Normal(intercept_prior_mean,
//! intercept_prior_sd)` and `coefficient ~ Normal(coefficient_prior_mean, coefficient_prior_sd)`.
//! The field is not centred, so only the priors separate its level from the intercept. A
//! complete fit claims `experimental_single_pattern_latent_field`, otherwise
//! `diagnostic_only_nonconverged`.

use super::posterior_validation::{diagnostics_pass, scalar_valid};
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::GriddedLgcpFitWorkerRequest;
use crate::GriddedLgcpPosterior;
use crate::GriddedLgcpPosteriorPredictive;
use crate::McseScalarSummary;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;
use marklab_data::ArbitraryWindowIppWindowSummary;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputIdentity {
    pub events_digest: String,
    pub event_membership_digest: String,
    pub quadrature_digest: String,
    pub window_logical_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalNode {
    pub node_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub weight_um2: f64,
    pub covariate: f64,
    pub offset: f64,
    pub count: u64,
}

#[derive(Debug, Serialize)]
pub struct WorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub source_request_sha256: String,
    pub source_request: GriddedLgcpFitWorkerRequest,
    pub input: InputIdentity,
    pub window: ArbitraryWindowIppWindowSummary,
    pub nodes: Vec<PhysicalNode>,
    pub physical_covariance: Vec<f64>,
    pub physical_cholesky: Vec<f64>,
    pub neighbor_pairs: Vec<[usize; 2]>,
    pub neighbor_radius_um: f64,
    pub maximum_neighbor_pairs: usize,
    pub maximum_events: usize,
    pub maximum_quadrature_nodes: usize,
    pub covariance_sha256: String,
    pub maximum_draw_node_work: u64,
    pub dense_factorization_work_units: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultModel {
    family: String,
    coordinate_unit: String,
    window: String,
    quadrature: String,
    covariates: String,
    latent_field: String,
    kernel: String,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    likelihood: String,
    backend_adapter: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultNode {
    pub node_id: String,
    x_um: f64,
    y_um: f64,
    weight_um2: f64,
    covariate: f64,
    offset: f64,
    count: u64,
    pub latent_effect: McseScalarSummary,
    intensity_per_um2: McseScalarSummary,
    pub expected_count: McseScalarSummary,
    pearson_residual: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialPpcSummary {
    observed: f64,
    replicated_mean: f64,
    replicated_sd: f64,
    probability_replicated_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialPpc {
    replicate_count: u32,
    prediction_seed: u64,
    neighbor_radius_um: f64,
    neighbor_pair_count: usize,
    node_density_variance: SpatialPpcSummary,
    neighbor_density_mean_absolute_difference: SpatialPpcSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FitResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub source_backend: WorkerBackend,
    model: ResultModel,
    pub input: InputIdentity,
    window: ArbitraryWindowIppWindowSummary,
    observed_event_count: usize,
    quadrature_node_count: usize,
    quadrature_weight_um2: f64,
    sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: GriddedLgcpPosterior,
    pub nodes: Vec<ResultNode>,
    pub diagnostics: PosteriorDiagnostics,
    posterior_predictive: GriddedLgcpPosteriorPredictive,
    pub spatial_posterior_predictive: Option<SpatialPpc>,
    seed: u64,
    pub request_sha256: String,
    source_request_sha256: String,
    covariance_sha256: String,
    draw_node_work: u64,
    maximum_draw_node_work: u64,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    maximum_predictive_points: u64,
    maximum_neighbor_pairs: usize,
    dense_covariance_elements: usize,
    dense_factorization_work_units: u64,
    statistical_unit: StatisticalUnit,
    null_model: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: ClaimStatus,
}

pub fn validate(
    result: &FitResult,
    request: &WorkerRequest,
    request_sha256: &str,
) -> Result<(), BayesError> {
    let draws = u64::from(request.source_request.sampling.chains)
        * u64::from(request.source_request.sampling.draws_per_chain);
    let nodes_valid = result
        .nodes
        .iter()
        .zip(&request.nodes)
        .all(|(result, input)| {
            result.node_id == input.node_id
                && equal(result.x_um, input.x_um)
                && equal(result.y_um, input.y_um)
                && equal(result.weight_um2, input.weight_um2)
                && equal(result.covariate, input.covariate)
                && equal(result.offset, input.offset)
                && result.count == input.count
                && scalar_valid(&result.latent_effect, false)
                && scalar_valid(&result.intensity_per_um2, true)
                && scalar_valid(&result.expected_count, true)
                && scalar_valid(&result.pearson_residual, false)
        });
    let diagnostics_pass = diagnostics_pass(
        &result.diagnostics,
        &request.source_request.diagnostic_policy,
    );
    let spatial_summary_valid = |summary: &SpatialPpcSummary| {
        [
            summary.observed,
            summary.replicated_mean,
            summary.replicated_sd,
            summary.probability_replicated_at_least_observed,
        ]
        .into_iter()
        .all(f64::is_finite)
            && summary.observed >= 0.0
            && summary.replicated_mean >= 0.0
            && summary.replicated_sd >= 0.0
            && (0.0..=1.0).contains(&summary.probability_replicated_at_least_observed)
    };
    let spatial_ppc_valid = result
        .spatial_posterior_predictive
        .as_ref()
        .is_some_and(|spatial| {
            spatial.replicate_count == request.source_request.prediction.replicates
                && spatial.prediction_seed == request.source_request.prediction.seed
                && equal(spatial.neighbor_radius_um, request.neighbor_radius_um)
                && spatial.neighbor_pair_count == request.neighbor_pairs.len()
                && spatial_summary_valid(&spatial.node_density_variance)
                && spatial_summary_valid(&spatial.neighbor_density_mean_absolute_difference)
        });
    if result.format != "marklab.bayesian_arbitrary_window_lgcp_fit"
        || result.version != 1
        || !result.backend.matches(&request.backend)
        || !result
            .source_backend
            .matches(&request.source_request.backend)
        || result.input.events_digest != request.input.events_digest
        || result.input.event_membership_digest != request.input.event_membership_digest
        || result.input.quadrature_digest != request.input.quadrature_digest
        || result.input.window_logical_digest != request.input.window_logical_digest
        || result.window.logical_digest != request.window.logical_digest
        || !equal(result.window.area_um2, request.window.area_um2)
        || !equal(result.window.perimeter_um, request.window.perimeter_um)
        || result.window.bounds_um != request.window.bounds_um
        || result.window.component_count != request.window.component_count
        || result.window.hole_count != request.window.hole_count
        || result.window.ring_count != request.window.ring_count
        || result.window.vertex_count != request.window.vertex_count
        || result.observed_event_count
            != request
                .nodes
                .iter()
                .map(|node| node.count as usize)
                .sum::<usize>()
        || result.quadrature_node_count != request.nodes.len()
        || result.nodes.len() != request.nodes.len()
        || !equal(
            result.quadrature_weight_um2,
            request.nodes.iter().map(|node| node.weight_um2).sum(),
        )
        || result.sampling.chains != request.source_request.sampling.chains
        || result.sampling.tune_per_chain != request.source_request.sampling.tune_per_chain
        || result.sampling.draws_per_chain != request.source_request.sampling.draws_per_chain
        || result.sampling.completed_draws != draws
        || (result.fit_state == FitState::Complete) != diagnostics_pass
        || !scalar_valid(&result.posterior.intercept, false)
        || !scalar_valid(&result.posterior.coefficient, false)
        || !nodes_valid
        || result.posterior_predictive.observed_total_count as usize != result.observed_event_count
        || ![
            result.posterior_predictive.replicated_total_mean,
            result.posterior_predictive.replicated_total_sd,
            result.posterior_predictive.replicated_zero_cells_mean,
        ]
        .into_iter()
        .all(f64::is_finite)
        || result.posterior_predictive.replicated_total_mean < 0.0
        || result.posterior_predictive.replicated_total_sd < 0.0
        || result.posterior_predictive.replicated_zero_cells_mean < 0.0
        || result.seed != request.source_request.sampling.seed
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != request.source_request_sha256
        || result.covariance_sha256 != request.covariance_sha256
        || result.draw_node_work != draws * request.nodes.len() as u64
        || result.draw_node_work > result.maximum_draw_node_work
        || result.maximum_draw_node_work != request.maximum_draw_node_work
        || result.maximum_events != request.maximum_events
        || result.maximum_quadrature_nodes != request.maximum_quadrature_nodes
        || result.maximum_predictive_points
            != request.source_request.prediction.maximum_total_points
        || result.maximum_neighbor_pairs != request.maximum_neighbor_pairs
        || result.dense_covariance_elements != request.nodes.len() * request.nodes.len()
        || result.dense_factorization_work_units != request.dense_factorization_work_units
        || result.statistical_unit != StatisticalUnit::OneObservedPointPattern
        || result.null_model != NullHypothesis::NoneDescriptiveLatentIntensityModel.as_str()
        || result.assumptions
            != [
                "event_to_weighted_node_membership_is_exact_and_complete",
                "quadrature_weights_partition_the_exact_window_area",
                "matern_amplitude_and_length_scale_are_fixed_not_inferred",
                "weighted_nodes_are_a_recorded_coarse_window_approximation",
                "latent_intensity_is_not_point_interaction_or_attraction",
            ]
        || result.finite_result_policy != "nonconverged_is_diagnostic_only_reject_non_finite"
        || result.model.window != "exact_multipolygon"
        || result.model.kernel != "fixed_matern_3_2_on_physical_quadrature_representatives"
        || !equal(
            result.model.field_amplitude,
            request.source_request.model.construction.field_amplitude,
        )
        || !equal(
            result.model.field_length_scale_um,
            request
                .source_request
                .model
                .construction
                .field_length_scale_um,
        )
        || !equal(
            result.model.jitter,
            request.source_request.model.construction.jitter,
        )
        || !equal(
            result.model.intercept_prior_mean,
            request
                .source_request
                .model
                .construction
                .intercept_prior_mean,
        )
        || !equal(
            result.model.intercept_prior_sd,
            request.source_request.model.construction.intercept_prior_sd,
        )
        || !equal(
            result.model.coefficient_prior_mean,
            request
                .source_request
                .model
                .construction
                .coefficient_prior_mean,
        )
        || !equal(
            result.model.coefficient_prior_sd,
            request
                .source_request
                .model
                .construction
                .coefficient_prior_sd,
        )
        || (result.fit_state == FitState::Complete) != spatial_ppc_valid
        || (result.fit_state != FitState::Complete && result.spatial_posterior_predictive.is_some())
        || (result.fit_state == FitState::Complete
            && result.claim_status != ClaimStatus::ExperimentalSinglePatternLatentField)
        || (result.fit_state != FitState::Complete
            && result.claim_status != ClaimStatus::DiagnosticOnlyNonconverged)
    {
        return Err(BayesError::WorkerContract(
            "arbitrary-window LGCP identity, diagnostics, or physical result differs".into(),
        ));
    }
    Ok(())
}

impl FitResult {
    pub fn validate_for(
        &self,
        request: &WorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        validate(self, request, request_sha256)
    }
}

fn equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12 * left.abs().max(right.abs()).max(1.0)
}

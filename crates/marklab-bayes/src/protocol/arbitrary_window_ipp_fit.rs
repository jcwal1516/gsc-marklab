//! Log-linear inhomogeneous Poisson process fitted to one point pattern in an exact window.
//!
//! The log likelihood is `sum_i eta(s_i) - sum_k w_k exp(eta_k)` with
//! `eta = intercept + coefficient * covariate + offset`. The window integral is replaced by a sum
//! over positive-area quadrature nodes whose weights `w_k` (um^2) partition the exact window
//! area; events contribute their own covariate and offset. Priors are
//! `intercept ~ Normal(intercept_prior_mean, intercept_prior_sd)` and
//! `coefficient ~ Normal(coefficient_prior_mean, coefficient_prior_sd)` (standard deviations);
//! there is no latent field. The PyMC and NumPyro workers receive each node as a unit-area cell
//! whose offset includes `ln(w_k)`. `fit_state` is complete only when prior and posterior draws
//! are finite and R-hat, bulk and tail ESS, E-BFMI, divergences and tree-depth hits meet the
//! request policy; the claim is then `experimental_single_pattern_association`, otherwise
//! `diagnostic_only_nonconverged`.

use crate::BayesError;
use crate::FitState;
use crate::InhomogeneousPoissonFitWorkerRequest;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::ScalarSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use marklab_data::ArbitraryWindowIppQuadratureNode;
use marklab_data::ArbitraryWindowIppWindowSummary;
use serde::Deserialize;
use serde::Serialize;

pub struct ValidationContext<'a> {
    pub request: &'a InhomogeneousPoissonFitWorkerRequest,
    pub request_sha256: &'a str,
    pub input_identity: &'a ArbitraryWindowIppFitInputIdentity,
    pub window_logical_digest: String,
    pub window_area_um2: f64,
    pub observed_event_count: usize,
    pub quadrature: &'a [ArbitraryWindowIppQuadratureNode],
    pub maximum_draw_node_work: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppFitInputIdentity {
    pub events_digest: String,
    pub quadrature_digest: String,
    pub window_logical_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppFitModel {
    pub family: String,
    pub coordinate_unit: String,
    pub window: String,
    pub quadrature: String,
    pub covariates: String,
    pub likelihood: String,
    pub backend_adapter: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppFitNodeSummary {
    pub node_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub weight_um2: f64,
    pub covariate: f64,
    pub offset: f64,
    pub intensity_per_um2: ScalarSummary,
    pub expected_count: ScalarSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppFitPredictive {
    pub observed_total_count: u64,
    pub total_expected_count_mean: f64,
    pub replicated_total_mean: f64,
    pub replicated_total_sd: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppFitPosterior {
    pub intercept: ScalarSummary,
    pub coefficient: ScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppFitResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: ArbitraryWindowIppFitModel,
    pub input: ArbitraryWindowIppFitInputIdentity,
    pub window: ArbitraryWindowIppWindowSummary,
    pub observed_event_count: usize,
    pub quadrature_node_count: usize,
    pub quadrature_weight_um2: f64,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: ArbitraryWindowIppFitPosterior,
    pub nodes: Vec<ArbitraryWindowIppFitNodeSummary>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: ArbitraryWindowIppFitPredictive,
    pub seed: u64,
    pub request_sha256: String,
    pub maximum_draw_node_work: u64,
    pub statistical_unit: StatisticalUnit,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

impl ArbitraryWindowIppFitResult {
    pub fn validate(&self, expected: &ValidationContext<'_>) -> Result<(), BayesError> {
        let expected_draws = u64::from(expected.request.sampling.chains)
            * u64::from(expected.request.sampling.draws_per_chain);
        let node_identity_matches =
            self.nodes
                .iter()
                .zip(expected.quadrature)
                .all(|(result, input)| {
                    result.node_id == input.node_id
                        && equal(result.x_um, input.x_um)
                        && equal(result.y_um, input.y_um)
                        && equal(result.weight_um2, input.weight_um2)
                        && equal(result.covariate, input.covariate)
                        && equal(result.offset, input.offset)
                        && scalar_valid(&result.intensity_per_um2, true)
                        && scalar_valid(&result.expected_count, true)
                });
        let diagnostics_pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= expected.request.diagnostic_policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= expected.request.diagnostic_policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= expected.request.diagnostic_policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= expected.request.diagnostic_policy.minimum_ebfmi
            && self.diagnostics.divergences
                <= expected.request.diagnostic_policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits
                <= expected.request.diagnostic_policy.maximum_tree_depth_hits;
        if self.format != "marklab.bayesian_arbitrary_window_ipp_fit"
            || self.version != 1
            || !self.backend.matches(&expected.request.backend)
            || self.request_sha256 != expected.request_sha256
            || self.input.events_digest != expected.input_identity.events_digest
            || self.input.quadrature_digest != expected.input_identity.quadrature_digest
            || self.input.window_logical_digest != expected.window_logical_digest
            || self.window.logical_digest != expected.window_logical_digest
            || self.observed_event_count != expected.observed_event_count
            || self.quadrature_node_count != expected.quadrature.len()
            || self.nodes.len() != expected.quadrature.len()
            || !equal(self.quadrature_weight_um2, expected.window_area_um2)
            || self.sampling.chains != expected.request.sampling.chains
            || self.sampling.tune_per_chain != expected.request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != expected.request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
            || self.seed != expected.request.sampling.seed
            || self.maximum_draw_node_work != expected.maximum_draw_node_work
            || !scalar_valid(&self.posterior.intercept, false)
            || !scalar_valid(&self.posterior.coefficient, false)
            || !node_identity_matches
            || ![
                self.posterior_predictive.total_expected_count_mean,
                self.posterior_predictive.replicated_total_mean,
                self.posterior_predictive.replicated_total_sd,
            ]
            .into_iter()
            .all(f64::is_finite)
            || self.posterior_predictive.total_expected_count_mean <= 0.0
            || self.posterior_predictive.replicated_total_mean < 0.0
            || self.posterior_predictive.replicated_total_sd < 0.0
            || (self.fit_state == FitState::Complete) != diagnostics_pass
            || self.statistical_unit != StatisticalUnit::OneObservedPointPattern
            || self.finite_result_policy != "nonconverged_is_diagnostic_only_reject_non_finite"
            || (self.fit_state == FitState::Complete
                && self.claim_status != ClaimStatus::ExperimentalSinglePatternAssociation)
            || (self.fit_state != FitState::Complete
                && self.claim_status != ClaimStatus::DiagnosticOnlyNonconverged)
        {
            return Err(BayesError::WorkerContract(
                "arbitrary-window IPP fit result identity or diagnostics differ".into(),
            ));
        }
        Ok(())
    }
}

pub fn divide_summary(value: ScalarSummary, divisor: f64) -> ScalarSummary {
    ScalarSummary {
        mean: value.mean / divisor,
        sd: value.sd / divisor,
        interval_lower: value.interval_lower / divisor,
        interval_upper: value.interval_upper / divisor,
    }
}

fn scalar_valid(value: &ScalarSummary, positive: bool) -> bool {
    [
        value.mean,
        value.sd,
        value.interval_lower,
        value.interval_upper,
    ]
    .into_iter()
    .all(f64::is_finite)
        && value.sd > 0.0
        && value.interval_lower <= value.interval_upper
        && (!positive || (value.mean > 0.0 && value.interval_lower >= 0.0))
}

fn equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12 * left.abs().max(right.abs()).max(1.0)
}

//! Typed messages and scientific checks for arbitrary window ipp agreement.
use crate::InhomogeneousPoissonFitWorkerRequest;

use super::arbitrary_window_ipp_fit::ArbitraryWindowIppFitPosterior;
use super::arbitrary_window_ipp_fit::ArbitraryWindowIppFitResult;
use crate::backend_versions::JAX_VERSION;
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use serde::Deserialize;
use serde::Serialize;
#[derive(Serialize)]
pub struct NumpyroRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: &'a str,
    pub source_request: &'a crate::InhomogeneousPoissonFitWorkerRequest,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    pub fit_state: FitState,
    sampling: SamplingSummary,
    posterior: ArbitraryWindowIppFitPosterior,
    diagnostics: PosteriorDiagnostics,
    total_expected_count_mean: f64,
    replicated_total_mean: f64,
    replicated_total_sd: f64,
}

pub fn validate_numpyro(
    result: &NumpyroResult,
    request: &NumpyroRequest<'_>,
    request_sha256: &str,
    source_request: &InhomogeneousPoissonFitWorkerRequest,
    source_request_sha256: &str,
) -> Result<(), BayesError> {
    let diagnostics_pass = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= source_request.diagnostic_policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= source_request.diagnostic_policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= source_request.diagnostic_policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi >= source_request.diagnostic_policy.minimum_ebfmi
        && result.diagnostics.divergences <= source_request.diagnostic_policy.maximum_divergences
        && result.diagnostics.max_tree_depth_hits
            <= source_request.diagnostic_policy.maximum_tree_depth_hits;
    let finite = [
        result.posterior.intercept.mean,
        result.posterior.intercept.sd,
        result.posterior.intercept.interval_lower,
        result.posterior.intercept.interval_upper,
        result.posterior.coefficient.mean,
        result.posterior.coefficient.sd,
        result.posterior.coefficient.interval_lower,
        result.posterior.coefficient.interval_upper,
        result.total_expected_count_mean,
        result.replicated_total_mean,
        result.replicated_total_sd,
    ]
    .into_iter()
    .all(f64::is_finite);
    if result.format != "marklab.numpyro_arbitrary_window_ipp_result"
        || result.version != 1
        || !result.backend.matches(&request.backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_request_sha256
        || result.sampling.chains != source_request.sampling.chains
        || result.sampling.tune_per_chain != source_request.sampling.tune_per_chain
        || result.sampling.draws_per_chain != source_request.sampling.draws_per_chain
        || result.sampling.completed_draws
            != u64::from(source_request.sampling.chains)
                * u64::from(source_request.sampling.draws_per_chain)
        || !finite
        || result.posterior.intercept.sd <= 0.0
        || result.posterior.coefficient.sd <= 0.0
        || result.total_expected_count_mean <= 0.0
        || result.replicated_total_mean < 0.0
        || result.replicated_total_sd < 0.0
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesError::WorkerContract(
            "NumPyro arbitrary-window IPP result identity or diagnostics differ".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ScalarComparison {
    pymc_mean: f64,
    numpyro_mean: f64,
    absolute_difference: f64,
    tolerance: f64,
    intervals_overlap: bool,
    pub passes: bool,
}

#[derive(Debug, Serialize)]
pub struct AgreementComparison {
    pub intercept: ScalarComparison,
    pub coefficient: ScalarComparison,
    pub total_expected_count: ScalarComparison,
    pub parameter_absolute_tolerance: f64,
    pub total_count_absolute_tolerance: f64,
    pub all_within_tolerance: bool,
}

pub fn compare(
    pymc: &ArbitraryWindowIppFitResult,
    numpyro: &NumpyroResult,
    parameter_tolerance: f64,
    total_tolerance: f64,
) -> AgreementComparison {
    AgreementComparison {
        intercept: scalar(
            &pymc.posterior.intercept,
            &numpyro.posterior.intercept,
            parameter_tolerance,
        ),
        coefficient: scalar(
            &pymc.posterior.coefficient,
            &numpyro.posterior.coefficient,
            parameter_tolerance,
        ),
        total_expected_count: ScalarComparison {
            pymc_mean: pymc.posterior_predictive.total_expected_count_mean,
            numpyro_mean: numpyro.total_expected_count_mean,
            absolute_difference: (pymc.posterior_predictive.total_expected_count_mean
                - numpyro.total_expected_count_mean)
                .abs(),
            tolerance: total_tolerance,
            intervals_overlap: true,
            passes: (pymc.posterior_predictive.total_expected_count_mean
                - numpyro.total_expected_count_mean)
                .abs()
                <= total_tolerance,
        },
        parameter_absolute_tolerance: parameter_tolerance,
        total_count_absolute_tolerance: total_tolerance,
        all_within_tolerance: false,
    }
}

fn scalar(
    pymc: &crate::ScalarSummary,
    numpyro: &crate::ScalarSummary,
    tolerance: f64,
) -> ScalarComparison {
    let absolute_difference = (pymc.mean - numpyro.mean).abs();
    let intervals_overlap = pymc.interval_lower <= numpyro.interval_upper
        && numpyro.interval_lower <= pymc.interval_upper;
    ScalarComparison {
        pymc_mean: pymc.mean,
        numpyro_mean: numpyro.mean,
        absolute_difference,
        tolerance,
        intervals_overlap,
        passes: absolute_difference <= tolerance && intervals_overlap,
    }
}

#[derive(Debug, Serialize)]
pub struct AgreementResult {
    pub format: String,
    pub version: u32,
    pub input: super::arbitrary_window_ipp_fit::ArbitraryWindowIppFitInputIdentity,
    pub pymc: ArbitraryWindowIppFitResult,
    pub numpyro: NumpyroResult,
    pub comparison: AgreementComparison,
    pub claim_status: ClaimStatus,
}

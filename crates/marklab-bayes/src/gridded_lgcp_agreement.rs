use serde::{Deserialize, Serialize};

use crate::agreement_metric::AgreementVerdict;
use crate::agreement_metric::{
    compare_elements, compare_parameter, ElementwiseAgreement, ParameterAgreement,
};
use crate::backend_versions::PYTHON_VERSION;
use crate::validation::is_sha256;
use crate::AgreementStatus;
use crate::ClaimStatus;
use crate::{
    gridded_lgcp_fit::{
        GriddedLgcpCellPosterior, GriddedLgcpFitInputIdentity, GriddedLgcpFitModelIr,
        GriddedLgcpFitWorkerRequest, GriddedLgcpFitWorkerResult, GriddedLgcpPosterior,
        GriddedLgcpPosteriorPredictive,
    },
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    json_sha256, BackendContract, BayesError, FitState, McseScalarSummary, PosteriorDiagnostics,
    SamplingSummary, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_gridded_lgcp_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_gridded_lgcp_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroGriddedLgcpWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: GriddedLgcpFitWorkerRequest,
}

impl NumpyroGriddedLgcpWorkerRequest {
    pub fn new(
        source_request: GriddedLgcpFitWorkerRequest,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !is_sha256(&environment_lock_sha256) || !is_sha256(&worker_sha256) {
            return Err(BayesError::InvalidSpec(
                "NumPyro LGCP environment and worker identities must be SHA-256 digests".into(),
            ));
        }
        let source_request_sha256 = json_sha256(&source_request)?;
        Ok(Self {
            format: REQUEST_FORMAT,
            version: 1,
            backend: BackendContract {
                name: "numpyro",
                version: NUMPYRO_VERSION,
                python_version: PYTHON_VERSION,
                environment_lock_sha256,
                worker_sha256,
            },
            jax_version: JAX_VERSION,
            source_request_sha256,
            source_request,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroGriddedLgcpWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GriddedLgcpPosterior,
    pub cells: Vec<GriddedLgcpCellPosterior>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: GriddedLgcpPosteriorPredictive,
    patterns: Vec<crate::GriddedLgcpPredictivePattern>,
}

impl NumpyroGriddedLgcpWorkerResult {
    pub fn into_validated_payload(
        self,
        request: &NumpyroGriddedLgcpWorkerRequest,
        request_sha256: &str,
    ) -> Result<GriddedLgcpFitWorkerResult, BayesError> {
        if self.format != RESULT_FORMAT
            || self.version != 1
            || self.jax_version != JAX_VERSION
            || self.source_request_sha256 != request.source_request_sha256
        {
            return Err(BayesError::WorkerContract(
                "NumPyro gridded LGCP result identity mismatch".into(),
            ));
        }
        let payload = GriddedLgcpFitWorkerResult {
            format: self.format,
            version: self.version,
            backend: self.backend,
            request_sha256: self.request_sha256,
            fit_state: self.fit_state,
            sampling: self.sampling,
            posterior: self.posterior,
            cells: self.cells,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            patterns: self.patterns,
        };
        payload.validate_for_backend(
            &request.source_request,
            request_sha256,
            RESULT_FORMAT,
            &request.backend,
        )?;
        Ok(payload)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GriddedLgcpAgreementPolicy {
    pub maximum_standardized_difference: f64,
    pub minimum_parameter_tolerance: f64,
    pub minimum_field_tolerance: f64,
}

impl GriddedLgcpAgreementPolicy {
    fn validate(self) -> Result<Self, BayesError> {
        if !self.maximum_standardized_difference.is_finite()
            || !(1.0..=10.0).contains(&self.maximum_standardized_difference)
            || !self.minimum_parameter_tolerance.is_finite()
            || self.minimum_parameter_tolerance <= 0.0
            || !self.minimum_field_tolerance.is_finite()
            || self.minimum_field_tolerance <= 0.0
        {
            return Err(BayesError::InvalidSpec(
                "gridded LGCP agreement controls are invalid".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpAgreementComparison {
    pub intercept: ParameterAgreement,
    pub coefficient: ParameterAgreement,
    pub latent_effect: ElementwiseAgreement,
    pub expected_count: ElementwiseAgreement,
    pub cell_count: usize,
    pub maximum_standardized_difference: f64,
    pub minimum_parameter_tolerance: f64,
    pub minimum_field_tolerance: f64,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpBackendSummary {
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GriddedLgcpPosterior,
    pub cells: Vec<GriddedLgcpCellPosterior>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: GriddedLgcpPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: GriddedLgcpFitModelIr,
    pub input: GriddedLgcpFitInputIdentity,
    pub covariance_sha256: String,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub pymc: GriddedLgcpBackendSummary,
    pub numpyro: GriddedLgcpBackendSummary,
    pub comparison: GriddedLgcpAgreementComparison,
    pub seed: u64,
    pub claim_status: ClaimStatus,
}

impl GriddedLgcpAgreementResult {
    pub fn new(
        source_request: GriddedLgcpFitWorkerRequest,
        input: GriddedLgcpFitInputIdentity,
        pymc: GriddedLgcpFitWorkerResult,
        numpyro: GriddedLgcpFitWorkerResult,
        policy: GriddedLgcpAgreementPolicy,
    ) -> Result<Self, BayesError> {
        let policy = policy.validate()?;
        if pymc.cells.len() != numpyro.cells.len() || pymc.cells.len() != source_request.cells.len()
        {
            return Err(BayesError::WorkerContract(
                "gridded LGCP backend cell dimensions differ".into(),
            ));
        }
        let intercept = compare_parameter(
            &pymc.posterior.intercept,
            &numpyro.posterior.intercept,
            policy.maximum_standardized_difference,
            policy.minimum_parameter_tolerance,
        );
        let coefficient = compare_parameter(
            &pymc.posterior.coefficient,
            &numpyro.posterior.coefficient,
            policy.maximum_standardized_difference,
            policy.minimum_parameter_tolerance,
        );
        let latent_effect = compare_field(
            &pymc.cells,
            &numpyro.cells,
            |cell| cell.latent_effect,
            policy,
        );
        let expected_count = compare_field(
            &pymc.cells,
            &numpyro.cells,
            |cell| cell.expected_count,
            policy,
        );
        let agrees =
            intercept.passes && coefficient.passes && latent_effect.passes && expected_count.passes;
        let verdict = AgreementVerdict::new(pymc.fit_state, numpyro.fit_state, agrees);
        Ok(Self {
            format: "marklab.bayesian_gridded_lgcp_cross_backend_agreement",
            version: 1,
            model: source_request.model,
            input,
            covariance_sha256: source_request.covariance_sha256,
            fit_state: verdict.fit_state,
            agreement_status: verdict.agreement_status,
            comparison: GriddedLgcpAgreementComparison {
                intercept,
                coefficient,
                latent_effect,
                expected_count,
                cell_count: pymc.cells.len(),
                maximum_standardized_difference: policy.maximum_standardized_difference,
                minimum_parameter_tolerance: policy.minimum_parameter_tolerance,
                minimum_field_tolerance: policy.minimum_field_tolerance,
            },
            pymc: summarize(pymc),
            numpyro: summarize(numpyro),
            seed: source_request.sampling.seed,
            claim_status: verdict.claim_status,
        })
    }
}

fn summarize(result: GriddedLgcpFitWorkerResult) -> GriddedLgcpBackendSummary {
    GriddedLgcpBackendSummary {
        backend: result.backend,
        request_sha256: result.request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        posterior: result.posterior,
        cells: result.cells,
        diagnostics: result.diagnostics,
        posterior_predictive: result.posterior_predictive,
    }
}

fn compare_field(
    pymc: &[GriddedLgcpCellPosterior],
    numpyro: &[GriddedLgcpCellPosterior],
    select: fn(&GriddedLgcpCellPosterior) -> McseScalarSummary,
    policy: GriddedLgcpAgreementPolicy,
) -> ElementwiseAgreement {
    compare_elements(
        pymc.iter()
            .zip(numpyro)
            .map(|(left, right)| (select(left), select(right))),
        policy.maximum_standardized_difference,
        policy.minimum_field_tolerance,
    )
}

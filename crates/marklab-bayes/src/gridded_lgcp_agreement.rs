use crate::validation::is_lower_hex_sha256 as is_sha256;

use serde::{Deserialize, Serialize};

use crate::{
    gridded_lgcp_fit::{
        GriddedLgcpCellPosterior, GriddedLgcpFitInputIdentity, GriddedLgcpFitModelIr,
        GriddedLgcpFitWorkerRequest, GriddedLgcpFitWorkerResult, GriddedLgcpPosterior,
        GriddedLgcpPosteriorPredictive,
    },
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    sha256_hex, BackendContract, BayesError, FitState, NormalMeanDiagnostics, SamplingSummary,
    WorkerBackend,
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
        let source_request_sha256 = sha256_hex(&serde_json::to_vec(&source_request)?);
        Ok(Self {
            format: REQUEST_FORMAT,
            version: 1,
            backend: BackendContract {
                name: "numpyro",
                version: NUMPYRO_VERSION,
                python_version: "3.12",
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
    pub diagnostics: NormalMeanDiagnostics,
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
pub struct LgcpScalarAgreement {
    pub pymc_mean: f64,
    pub numpyro_mean: f64,
    pub absolute_difference: f64,
    pub combined_mcse: f64,
    pub standardized_difference: f64,
    pub tolerance: f64,
    pub intervals_overlap: bool,
    pub passes: bool,
}

#[derive(Debug, Serialize)]
pub struct LgcpFieldAgreement {
    pub root_mean_square_difference: f64,
    pub maximum_absolute_difference: f64,
    pub maximum_standardized_difference: f64,
    pub all_intervals_overlap: bool,
    pub passes: bool,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpAgreementComparison {
    pub intercept: LgcpScalarAgreement,
    pub coefficient: LgcpScalarAgreement,
    pub latent_effect: LgcpFieldAgreement,
    pub expected_count: LgcpFieldAgreement,
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
    pub diagnostics: NormalMeanDiagnostics,
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
    pub agreement_status: &'static str,
    pub pymc: GriddedLgcpBackendSummary,
    pub numpyro: GriddedLgcpBackendSummary,
    pub comparison: GriddedLgcpAgreementComparison,
    pub seed: u64,
    pub claim_status: &'static str,
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
        let intercept = compare_scalar(
            &pymc.posterior.intercept,
            pymc.diagnostics.mcse_mean,
            &numpyro.posterior.intercept,
            numpyro.diagnostics.mcse_mean,
            policy.maximum_standardized_difference,
            policy.minimum_parameter_tolerance,
        );
        let coefficient = compare_scalar(
            &pymc.posterior.coefficient,
            pymc.diagnostics.mcse_mean,
            &numpyro.posterior.coefficient,
            numpyro.diagnostics.mcse_mean,
            policy.maximum_standardized_difference,
            policy.minimum_parameter_tolerance,
        );
        let latent_effect = compare_field(
            &pymc.cells,
            &numpyro.cells,
            |cell| &cell.latent_effect,
            pymc.diagnostics.ess_bulk,
            numpyro.diagnostics.ess_bulk,
            policy,
        );
        let expected_count = compare_field(
            &pymc.cells,
            &numpyro.cells,
            |cell| &cell.expected_count,
            pymc.diagnostics.ess_bulk,
            numpyro.diagnostics.ess_bulk,
            policy,
        );
        let fits_complete =
            pymc.fit_state == FitState::Complete && numpyro.fit_state == FitState::Complete;
        let agrees =
            intercept.passes && coefficient.passes && latent_effect.passes && expected_count.passes;
        let available = fits_complete && agrees;
        Ok(Self {
            format: "marklab.bayesian_gridded_lgcp_cross_backend_agreement",
            version: 1,
            model: source_request.model,
            input,
            covariance_sha256: source_request.covariance_sha256,
            fit_state: if fits_complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            agreement_status: if available {
                "agree_within_monte_carlo_error"
            } else if fits_complete {
                "diagnostic_only_backend_disagreement"
            } else {
                "diagnostic_only_nonconverged"
            },
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
            claim_status: if available {
                "experimental_cross_backend_validation"
            } else {
                "diagnostic_only_cross_backend_validation"
            },
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

fn compare_scalar(
    pymc: &crate::SarScalarSummary,
    pymc_mcse: f64,
    numpyro: &crate::SarScalarSummary,
    numpyro_mcse: f64,
    maximum_standardized_difference: f64,
    minimum_tolerance: f64,
) -> LgcpScalarAgreement {
    let absolute_difference = (pymc.mean - numpyro.mean).abs();
    let combined_mcse = pymc_mcse.hypot(numpyro_mcse);
    let standardized_difference = standardized(absolute_difference, combined_mcse);
    let tolerance = minimum_tolerance.max(maximum_standardized_difference * combined_mcse);
    let intervals_overlap = overlap(pymc, numpyro);
    LgcpScalarAgreement {
        pymc_mean: pymc.mean,
        numpyro_mean: numpyro.mean,
        absolute_difference,
        combined_mcse,
        standardized_difference,
        tolerance,
        intervals_overlap,
        passes: intervals_overlap && absolute_difference <= tolerance,
    }
}

fn compare_field<F>(
    pymc: &[GriddedLgcpCellPosterior],
    numpyro: &[GriddedLgcpCellPosterior],
    select: F,
    pymc_ess: f64,
    numpyro_ess: f64,
    policy: GriddedLgcpAgreementPolicy,
) -> LgcpFieldAgreement
where
    F: Fn(&GriddedLgcpCellPosterior) -> &crate::SarScalarSummary,
{
    let mut squared = 0.0;
    let mut maximum_absolute: f64 = 0.0;
    let mut maximum_standardized: f64 = 0.0;
    let mut all_intervals_overlap = true;
    let mut passes = true;
    for (pymc_cell, numpyro_cell) in pymc.iter().zip(numpyro) {
        let pymc_summary = select(pymc_cell);
        let numpyro_summary = select(numpyro_cell);
        let difference = (pymc_summary.mean - numpyro_summary.mean).abs();
        let combined_mcse =
            (pymc_summary.sd / pymc_ess.sqrt()).hypot(numpyro_summary.sd / numpyro_ess.sqrt());
        let standardized_difference = standardized(difference, combined_mcse);
        let tolerance = policy
            .minimum_field_tolerance
            .max(policy.maximum_standardized_difference * combined_mcse);
        let intervals_overlap = overlap(pymc_summary, numpyro_summary);
        squared += difference * difference;
        maximum_absolute = maximum_absolute.max(difference);
        maximum_standardized = maximum_standardized.max(standardized_difference);
        all_intervals_overlap &= intervals_overlap;
        passes &= intervals_overlap && difference <= tolerance;
    }
    LgcpFieldAgreement {
        root_mean_square_difference: (squared / pymc.len() as f64).sqrt(),
        maximum_absolute_difference: maximum_absolute,
        maximum_standardized_difference: maximum_standardized,
        all_intervals_overlap,
        passes,
    }
}

fn standardized(difference: f64, mcse: f64) -> f64 {
    if mcse > 0.0 {
        difference / mcse
    } else if difference == 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}

fn overlap(left: &crate::SarScalarSummary, right: &crate::SarScalarSummary) -> bool {
    left.interval_lower <= right.interval_upper && right.interval_lower <= left.interval_upper
}

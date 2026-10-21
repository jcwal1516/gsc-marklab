use crate::is_worker_timeout;
use crate::validation::all_finite as finite;
use crate::validation::is_sha256;

use serde::{Deserialize, Serialize};

use crate::backend_versions::PYTHON_VERSION;
use crate::sbc::{validate_sbc_replicates, SbcCalibration, SbcReplicateRow, SbcSamplerRow};
use crate::ClaimStatus;
use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    BackendContract, BayesError, FitState, GriddedLgcpFitModelIr, GriddedLgcpFitWorkerRequest,
    NutsSamplingSpec, SbcParameterDiagnostics, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_gridded_lgcp_sbc_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_gridded_lgcp_sbc_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct GriddedLgcpSbcCalibrationPolicy {
    pub replicates: u32,
    pub rank_bins: u32,
    pub interval_probability: f64,
    pub latent_cell_index: u32,
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

impl SbcCalibration for GriddedLgcpSbcCalibrationPolicy {
    fn replicates(&self) -> u32 {
        self.replicates
    }
    fn rank_bins(&self) -> u32 {
        self.rank_bins
    }
    fn passes(&self, diagnostic: &SbcParameterDiagnostics) -> bool {
        diagnostic.rank_uniformity_p_value >= self.minimum_rank_uniformity_p_value
            && (self.minimum_coverage..=self.maximum_coverage).contains(&diagnostic.coverage_90)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GriddedLgcpSbcResourceLimits {
    pub maximum_replicates: u32,
    pub maximum_simulated_cells: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroGriddedLgcpSbcWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: GriddedLgcpFitWorkerRequest,
    pub calibration: GriddedLgcpSbcCalibrationPolicy,
    pub resources: GriddedLgcpSbcResourceLimits,
}

impl NumpyroGriddedLgcpSbcWorkerRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_request: GriddedLgcpFitWorkerRequest,
        replicates: u32,
        minimum_rank_uniformity_p_value: f64,
        minimum_coverage: f64,
        maximum_coverage: f64,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if !(20..=100).contains(&replicates)
            || !minimum_rank_uniformity_p_value.is_finite()
            || !(0.0..0.1).contains(&minimum_rank_uniformity_p_value)
            || !minimum_coverage.is_finite()
            || !maximum_coverage.is_finite()
            || !(0.0..=1.0).contains(&minimum_coverage)
            || !(0.0..=1.0).contains(&maximum_coverage)
            || minimum_coverage > maximum_coverage
            || !is_sha256(&environment_lock_sha256)
            || !is_sha256(&worker_sha256)
            || !is_worker_timeout(timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "gridded LGCP SBC controls or identities are invalid".into(),
            ));
        }
        let simulated_cells = u64::from(replicates)
            .checked_mul(source_request.cells.len() as u64)
            .ok_or_else(|| BayesError::InvalidSpec("gridded LGCP SBC work overflow".into()))?;
        let maximum_simulated_cells = 3_600;
        let total_iterations = u64::from(replicates)
            .checked_mul(u64::from(source_request.sampling.chains))
            .and_then(|value| {
                value.checked_mul(u64::from(
                    source_request.sampling.tune_per_chain
                        + source_request.sampling.draws_per_chain,
                ))
            })
            .ok_or_else(|| {
                BayesError::InvalidSpec("gridded LGCP SBC iterations overflow".into())
            })?;
        let maximum_total_iterations = 1_000_000;
        if simulated_cells > maximum_simulated_cells || total_iterations > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "gridded LGCP SBC exceeds its simulation or iteration limit".into(),
            ));
        }
        let source_request_sha256 = crate::json_sha256(&source_request)?;
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
            calibration: GriddedLgcpSbcCalibrationPolicy {
                replicates,
                rank_bins: 10,
                interval_probability: 0.9,
                latent_cell_index: 0,
                minimum_rank_uniformity_p_value,
                minimum_coverage,
                maximum_coverage,
            },
            resources: GriddedLgcpSbcResourceLimits {
                maximum_replicates: 100,
                maximum_simulated_cells,
                maximum_total_iterations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpSbcReplicate {
    pub replicate: u32,
    pub true_intercept: f64,
    pub true_coefficient: f64,
    pub true_latent_cell: f64,
    pub intercept_rank: u32,
    pub coefficient_rank: u32,
    pub latent_cell_rank: u32,
    pub intercept_covered: bool,
    pub coefficient_covered: bool,
    pub latent_cell_covered: bool,
    pub r_hat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
    pub minimum_ebfmi: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
}

impl SbcReplicateRow for GriddedLgcpSbcReplicate {
    fn replicate(&self) -> u32 {
        self.replicate
    }
    fn ranks(&self) -> Vec<(u32, bool)> {
        vec![
            (self.intercept_rank, self.intercept_covered),
            (self.coefficient_rank, self.coefficient_covered),
            (self.latent_cell_rank, self.latent_cell_covered),
        ]
    }
    fn truths_valid(&self) -> bool {
        finite(&[
            self.true_intercept,
            self.true_coefficient,
            self.true_latent_cell,
        ])
    }
    fn sampler(&self) -> SbcSamplerRow {
        SbcSamplerRow {
            r_hat: self.r_hat,
            ess_bulk: self.ess_bulk,
            ess_tail: self.ess_tail,
            minimum_ebfmi: Some(self.minimum_ebfmi),
            divergences: self.divergences,
            max_tree_depth_hits: self.max_tree_depth_hits,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpSbcFailure {
    pub replicate: u32,
    pub reason: String,
}

impl crate::sbc::SbcFailureRow for GriddedLgcpSbcFailure {
    fn replicate(&self) -> u32 {
        self.replicate
    }
    fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpSbcDiagnostics {
    pub intercept: SbcParameterDiagnostics,
    pub coefficient: SbcParameterDiagnostics,
    pub latent_cell: SbcParameterDiagnostics,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroGriddedLgcpSbcWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub replicates: Vec<GriddedLgcpSbcReplicate>,
    pub failures: Vec<GriddedLgcpSbcFailure>,
    pub diagnostics: GriddedLgcpSbcDiagnostics,
}

impl NumpyroGriddedLgcpSbcWorkerResult {
    pub fn validate(
        &self,
        request: &NumpyroGriddedLgcpSbcWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != RESULT_FORMAT
            || self.version != 1
            || !self.backend.matches(&request.backend)
            || self.jax_version != JAX_VERSION
            || self.request_sha256 != request_sha256
            || self.replicates.len() + self.failures.len()
                != request.calibration.replicates as usize
        {
            return Err(BayesError::WorkerContract(
                "gridded LGCP SBC result identity or dimensions mismatch".into(),
            ));
        }
        let posterior_draws = request.source_request.sampling.chains
            * request.source_request.sampling.draws_per_chain;
        validate_sbc_replicates(
            &self.replicates,
            &self.failures,
            &[
                &self.diagnostics.intercept,
                &self.diagnostics.coefficient,
                &self.diagnostics.latent_cell,
            ],
            &request.calibration,
            posterior_draws,
            &request.source_request.diagnostic_policy,
            self.fit_state,
        )
        .map_err(|violation| violation.error("gridded LGCP"))
    }

    pub fn into_result(self, request: NumpyroGriddedLgcpSbcWorkerRequest) -> GriddedLgcpSbcResult {
        GriddedLgcpSbcResult {
            format: "marklab.bayesian_gridded_lgcp_sbc",
            version: 1,
            backend: self.backend,
            jax_version: self.jax_version,
            model: request.source_request.model,
            covariance_sha256: request.source_request.covariance_sha256,
            sampling: request.source_request.sampling,
            calibration: request.calibration,
            resources: request.resources,
            fit_state: self.fit_state,
            replicates: self.replicates,
            failures: self.failures,
            diagnostics: self.diagnostics,
            claim_status: if self.fit_state == FitState::Complete {
                ClaimStatus::ExperimentalSimulationCalibration
            } else {
                ClaimStatus::DiagnosticOnlyFailedCalibration
            },
            request_sha256: self.request_sha256,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpSbcResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub model: GriddedLgcpFitModelIr,
    pub covariance_sha256: String,
    pub sampling: NutsSamplingSpec,
    pub calibration: GriddedLgcpSbcCalibrationPolicy,
    pub resources: GriddedLgcpSbcResourceLimits,
    pub fit_state: FitState,
    pub replicates: Vec<GriddedLgcpSbcReplicate>,
    pub failures: Vec<GriddedLgcpSbcFailure>,
    pub diagnostics: GriddedLgcpSbcDiagnostics,
    pub claim_status: ClaimStatus,
    pub request_sha256: String,
}

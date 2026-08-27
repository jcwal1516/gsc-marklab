use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    BackendContract, BayesError, FitState, GriddedLgcpFitModelIr, GriddedLgcpFitWorkerRequest,
    NutsSamplingSpec, WorkerBackend,
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
            || !(1..=3_600).contains(&timeout_seconds)
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
        let source_request_sha256 = crate::sha256_hex(&serde_json::to_vec(&source_request)?);
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

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpSbcFailure {
    pub replicate: u32,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpSbcParameterDiagnostics {
    pub rank_histogram: Vec<u32>,
    pub rank_uniformity_p_value: f64,
    pub coverage_90: f64,
    pub mean_normalized_rank: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GriddedLgcpSbcDiagnostics {
    pub intercept: GriddedLgcpSbcParameterDiagnostics,
    pub coefficient: GriddedLgcpSbcParameterDiagnostics,
    pub latent_cell: GriddedLgcpSbcParameterDiagnostics,
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
            || self.backend.name != request.backend.name
            || self.backend.version != request.backend.version
            || self.backend.python_version != request.backend.python_version
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
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
        let mut dispositions = BTreeSet::new();
        for replicate in &self.replicates {
            if replicate.replicate >= request.calibration.replicates
                || !dispositions.insert(replicate.replicate)
                || replicate.intercept_rank > posterior_draws
                || replicate.coefficient_rank > posterior_draws
                || replicate.latent_cell_rank > posterior_draws
                || !finite(&[
                    replicate.true_intercept,
                    replicate.true_coefficient,
                    replicate.true_latent_cell,
                    replicate.r_hat,
                    replicate.ess_bulk,
                    replicate.ess_tail,
                    replicate.minimum_ebfmi,
                ])
                || replicate.r_hat <= 0.0
                || replicate.ess_bulk <= 0.0
                || replicate.ess_tail <= 0.0
                || replicate.minimum_ebfmi < 0.0
            {
                return Err(BayesError::WorkerContract(
                    "gridded LGCP SBC replicate is invalid".into(),
                ));
            }
        }
        for failure in &self.failures {
            if failure.replicate >= request.calibration.replicates
                || !dispositions.insert(failure.replicate)
                || failure.reason.is_empty()
                || failure.reason.len() > 512
            {
                return Err(BayesError::WorkerContract(
                    "gridded LGCP SBC failure disposition is invalid".into(),
                ));
            }
        }
        if dispositions.len() != request.calibration.replicates as usize {
            return Err(BayesError::WorkerContract(
                "gridded LGCP SBC lacks complete replicate disposition".into(),
            ));
        }
        let diagnostics = [
            &self.diagnostics.intercept,
            &self.diagnostics.coefficient,
            &self.diagnostics.latent_cell,
        ];
        for diagnostic in diagnostics {
            if diagnostic.rank_histogram.len() != request.calibration.rank_bins as usize
                || diagnostic.rank_histogram.iter().sum::<u32>() != self.replicates.len() as u32
                || !finite(&[
                    diagnostic.rank_uniformity_p_value,
                    diagnostic.coverage_90,
                    diagnostic.mean_normalized_rank,
                ])
                || !(0.0..=1.0).contains(&diagnostic.rank_uniformity_p_value)
                || !(0.0..=1.0).contains(&diagnostic.coverage_90)
                || !(0.0..=1.0).contains(&diagnostic.mean_normalized_rank)
            {
                return Err(BayesError::WorkerContract(
                    "gridded LGCP SBC aggregate diagnostics are invalid".into(),
                ));
            }
        }
        let passes = self.failures.is_empty()
            && [
                &self.diagnostics.intercept,
                &self.diagnostics.coefficient,
                &self.diagnostics.latent_cell,
            ]
            .into_iter()
            .all(|diagnostic| {
                diagnostic.rank_uniformity_p_value
                    >= request.calibration.minimum_rank_uniformity_p_value
                    && (request.calibration.minimum_coverage..=request.calibration.maximum_coverage)
                        .contains(&diagnostic.coverage_90)
            });
        if (self.fit_state == FitState::Complete) != passes {
            return Err(BayesError::WorkerContract(
                "gridded LGCP SBC fit state disagrees with calibration gates".into(),
            ));
        }
        Ok(())
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
                "experimental_simulation_calibration"
            } else {
                "diagnostic_only_failed_calibration"
            },
            request_sha256: self.request_sha256,
        }
    }
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
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
    pub claim_status: &'static str,
    pub request_sha256: String,
}

use crate::is_worker_timeout;
use crate::validation::is_sha256;

use serde::{Deserialize, Serialize};

use crate::backend_versions::PYTHON_VERSION;
use crate::sbc::{
    validate_sbc_replicates, SbcCalibrationPolicy, SbcControls, SbcReplicateRow, SbcSamplerRow,
};
use crate::ClaimStatus;
use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    BackendContract, BayesError, FitState, HierarchicalSbcFailure, NutsSamplingSpec,
    SbcParameterDiagnostics, StudentTHierarchyModelIr, StudentTHierarchyWorkerRequest,
    WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_student_t_hierarchy_sbc_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_student_t_hierarchy_sbc_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct StudentTSbcResourceLimits {
    pub maximum_replicates: u32,
    pub maximum_simulated_observations: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroStudentTSbcWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: StudentTHierarchyWorkerRequest,
    pub calibration: SbcCalibrationPolicy,
    pub resources: StudentTSbcResourceLimits,
}

impl NumpyroStudentTSbcWorkerRequest {
    pub fn new(
        source_request: StudentTHierarchyWorkerRequest,
        controls: SbcControls,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        let calibration = SbcCalibrationPolicy::new(controls)
            .filter(|_| {
                is_sha256(&environment_lock_sha256)
                    && is_sha256(&worker_sha256)
                    && is_worker_timeout(timeout_seconds)
            })
            .ok_or_else(|| {
                BayesError::InvalidSpec("Student-t SBC controls or identities are invalid".into())
            })?;
        let replicates = calibration.replicates;
        let maximum_simulated_observations = 10_000_000;
        let simulated = u64::from(replicates)
            .checked_mul(source_request.observation_count() as u64)
            .ok_or_else(|| BayesError::InvalidSpec("Student-t SBC work overflow".into()))?;
        let maximum_total_iterations = 1_000_000;
        let total_iterations = u64::from(replicates)
            .checked_mul(u64::from(source_request.sampling.chains))
            .and_then(|value| {
                value.checked_mul(u64::from(
                    source_request.sampling.tune_per_chain
                        + source_request.sampling.draws_per_chain,
                ))
            })
            .ok_or_else(|| BayesError::InvalidSpec("Student-t SBC iterations overflow".into()))?;
        if simulated > maximum_simulated_observations || total_iterations > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "Student-t SBC exceeds observation or iteration limits".into(),
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
            calibration,
            resources: StudentTSbcResourceLimits {
                maximum_replicates: 100,
                maximum_simulated_observations,
                maximum_total_iterations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudentTSbcReplicate {
    pub replicate: u32,
    pub true_global_mean: f64,
    pub true_between_patient_sd: f64,
    pub true_observation_sd: f64,
    pub true_degrees_of_freedom: f64,
    pub global_mean_rank: u32,
    pub between_patient_sd_rank: u32,
    pub observation_sd_rank: u32,
    pub degrees_of_freedom_rank: u32,
    pub global_mean_covered: bool,
    pub between_patient_sd_covered: bool,
    pub observation_sd_covered: bool,
    pub degrees_of_freedom_covered: bool,
    pub r_hat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
    pub minimum_ebfmi: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
}

impl SbcReplicateRow for StudentTSbcReplicate {
    fn replicate(&self) -> u32 {
        self.replicate
    }

    fn ranks(&self) -> Vec<(u32, bool)> {
        vec![
            (self.global_mean_rank, self.global_mean_covered),
            (
                self.between_patient_sd_rank,
                self.between_patient_sd_covered,
            ),
            (self.observation_sd_rank, self.observation_sd_covered),
            (
                self.degrees_of_freedom_rank,
                self.degrees_of_freedom_covered,
            ),
        ]
    }

    fn truths_valid(&self) -> bool {
        [
            self.true_global_mean,
            self.true_between_patient_sd,
            self.true_observation_sd,
            self.true_degrees_of_freedom,
        ]
        .iter()
        .all(|value| value.is_finite())
            && self.true_between_patient_sd > 0.0
            && self.true_observation_sd > 0.0
            && self.true_degrees_of_freedom > 2.0
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
pub struct StudentTSbcDiagnostics {
    pub global_mean: SbcParameterDiagnostics,
    pub between_patient_sd: SbcParameterDiagnostics,
    pub observation_sd: SbcParameterDiagnostics,
    pub degrees_of_freedom: SbcParameterDiagnostics,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroStudentTSbcWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub replicates: Vec<StudentTSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: StudentTSbcDiagnostics,
}

impl NumpyroStudentTSbcWorkerResult {
    pub fn validate(
        &self,
        request: &NumpyroStudentTSbcWorkerRequest,
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
                "Student-t SBC result identity or dimensions mismatch".into(),
            ));
        }
        let posterior_draws = request.source_request.sampling.chains
            * request.source_request.sampling.draws_per_chain;
        validate_sbc_replicates(
            &self.replicates,
            &self.failures,
            &[
                &self.diagnostics.global_mean,
                &self.diagnostics.between_patient_sd,
                &self.diagnostics.observation_sd,
                &self.diagnostics.degrees_of_freedom,
            ],
            &request.calibration,
            posterior_draws,
            &request.source_request.diagnostic_policy,
            self.fit_state,
        )
        .map_err(|violation| violation.error("Student-t"))
    }

    pub fn into_result(self, request: NumpyroStudentTSbcWorkerRequest) -> StudentTSbcResult {
        StudentTSbcResult {
            format: "marklab.bayesian_student_t_hierarchy_sbc",
            version: 1,
            backend: self.backend,
            jax_version: self.jax_version,
            model: request.source_request.model,
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
pub struct StudentTSbcResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub model: StudentTHierarchyModelIr,
    pub sampling: NutsSamplingSpec,
    pub calibration: SbcCalibrationPolicy,
    pub resources: StudentTSbcResourceLimits,
    pub fit_state: FitState,
    pub replicates: Vec<StudentTSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: StudentTSbcDiagnostics,
    pub claim_status: ClaimStatus,
    pub request_sha256: String,
}

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    BackendContract, BayesError, FitState, HierarchicalSbcFailure,
    HierarchicalSbcParameterDiagnostics, NutsSamplingSpec, StudentTHierarchyModelIr,
    StudentTHierarchyWorkerRequest, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_student_t_hierarchy_sbc_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_student_t_hierarchy_sbc_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct StudentTSbcCalibrationPolicy {
    pub replicates: u32,
    pub rank_bins: u32,
    pub interval_probability: f64,
    pub maximum_tree_depth: u32,
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

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
    pub calibration: StudentTSbcCalibrationPolicy,
    pub resources: StudentTSbcResourceLimits,
}

impl NumpyroStudentTSbcWorkerRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_request: StudentTHierarchyWorkerRequest,
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
                "Student-t SBC controls or identities are invalid".into(),
            ));
        }
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
            calibration: StudentTSbcCalibrationPolicy {
                replicates,
                rank_bins: 10,
                interval_probability: 0.9,
                maximum_tree_depth: 12,
                minimum_rank_uniformity_p_value,
                minimum_coverage,
                maximum_coverage,
            },
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

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudentTSbcDiagnostics {
    pub global_mean: HierarchicalSbcParameterDiagnostics,
    pub between_patient_sd: HierarchicalSbcParameterDiagnostics,
    pub observation_sd: HierarchicalSbcParameterDiagnostics,
    pub degrees_of_freedom: HierarchicalSbcParameterDiagnostics,
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
                "Student-t SBC result identity or dimensions mismatch".into(),
            ));
        }
        let posterior_draws = request.source_request.sampling.chains
            * request.source_request.sampling.draws_per_chain;
        let mut dispositions = BTreeSet::new();
        for row in &self.replicates {
            if row.replicate >= request.calibration.replicates
                || !dispositions.insert(row.replicate)
                || [
                    row.global_mean_rank,
                    row.between_patient_sd_rank,
                    row.observation_sd_rank,
                    row.degrees_of_freedom_rank,
                ]
                .iter()
                .any(|rank| *rank > posterior_draws)
                || ![
                    row.true_global_mean,
                    row.true_between_patient_sd,
                    row.true_observation_sd,
                    row.true_degrees_of_freedom,
                    row.r_hat,
                    row.ess_bulk,
                    row.ess_tail,
                    row.minimum_ebfmi,
                ]
                .iter()
                .all(|value| value.is_finite())
                || row.true_between_patient_sd <= 0.0
                || row.true_observation_sd <= 0.0
                || row.true_degrees_of_freedom <= 2.0
                || row.r_hat > request.source_request.diagnostic_policy.maximum_r_hat
                || row.ess_bulk < request.source_request.diagnostic_policy.minimum_bulk_ess
                || row.ess_tail < request.source_request.diagnostic_policy.minimum_tail_ess
                || row.minimum_ebfmi < request.source_request.diagnostic_policy.minimum_ebfmi
                || row.divergences > request.source_request.diagnostic_policy.maximum_divergences
                || row.max_tree_depth_hits
                    > request
                        .source_request
                        .diagnostic_policy
                        .maximum_tree_depth_hits
            {
                return Err(BayesError::WorkerContract(
                    "Student-t SBC replicate is invalid".into(),
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
                    "Student-t SBC failure disposition is invalid".into(),
                ));
            }
        }
        let diagnostics = [
            &self.diagnostics.global_mean,
            &self.diagnostics.between_patient_sd,
            &self.diagnostics.observation_sd,
            &self.diagnostics.degrees_of_freedom,
        ];
        for diagnostic in diagnostics {
            if diagnostic.rank_histogram.len() != request.calibration.rank_bins as usize
                || diagnostic.rank_histogram.iter().sum::<u32>() != self.replicates.len() as u32
                || ![
                    diagnostic.rank_uniformity_p_value,
                    diagnostic.coverage_90,
                    diagnostic.mean_normalized_rank,
                ]
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            {
                return Err(BayesError::WorkerContract(
                    "Student-t SBC diagnostics are invalid".into(),
                ));
            }
        }
        let passes = self.failures.is_empty()
            && [
                &self.diagnostics.global_mean,
                &self.diagnostics.between_patient_sd,
                &self.diagnostics.observation_sd,
                &self.diagnostics.degrees_of_freedom,
            ]
            .into_iter()
            .all(|diagnostic| {
                diagnostic.rank_uniformity_p_value
                    >= request.calibration.minimum_rank_uniformity_p_value
                    && (request.calibration.minimum_coverage..=request.calibration.maximum_coverage)
                        .contains(&diagnostic.coverage_90)
            });
        if dispositions.len() != request.calibration.replicates as usize
            || (self.fit_state == FitState::Complete) != passes
        {
            return Err(BayesError::WorkerContract(
                "Student-t SBC disposition or fit state is invalid".into(),
            ));
        }
        Ok(())
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
                "experimental_simulation_calibration"
            } else {
                "diagnostic_only_failed_calibration"
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
    pub calibration: StudentTSbcCalibrationPolicy,
    pub resources: StudentTSbcResourceLimits,
    pub fit_state: FitState,
    pub replicates: Vec<StudentTSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: StudentTSbcDiagnostics,
    pub claim_status: &'static str,
    pub request_sha256: String,
}

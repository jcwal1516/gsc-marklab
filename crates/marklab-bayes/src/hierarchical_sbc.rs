use crate::is_worker_timeout;
use crate::validation::all_finite as finite;
use crate::validation::is_sha256;

use serde::{Deserialize, Serialize};

use crate::backend_versions::JAX_VERSION;
use crate::backend_versions::NUMPYRO_VERSION;
use crate::backend_versions::PYTHON_VERSION;
use crate::sbc::{validate_sbc_replicates, SbcCalibration, SbcReplicateRow, SbcSamplerRow};
use crate::ClaimStatus;
use crate::{
    hierarchical::{GaussianHierarchyModelIr, GaussianHierarchySpec, HierarchicalPatientData},
    BackendContract, BayesError, DiagnosticPolicy, FitState, GaussianHierarchyWorkerRequest,
    NutsSamplingSpec, SbcParameterDiagnostics, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_hierarchical_sbc_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_hierarchical_sbc_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct HierarchicalSbcCalibrationPolicy {
    pub replicates: u32,
    pub patient_count: u32,
    pub observations_per_patient: u32,
    pub rank_bins: u32,
    pub interval_probability: f64,
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

impl SbcCalibration for HierarchicalSbcCalibrationPolicy {
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
pub struct HierarchicalSbcResourceLimits {
    pub maximum_replicates: u32,
    pub maximum_simulated_observations: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroHierarchySbcWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub model: GaussianHierarchyModelIr,
    pub sampling: NutsSamplingSpec,
    pub calibration: HierarchicalSbcCalibrationPolicy,
    pub diagnostic_policy: DiagnosticPolicy,
    pub resources: HierarchicalSbcResourceLimits,
}

impl NumpyroHierarchySbcWorkerRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        global_prior_mean: f64,
        global_prior_sd: f64,
        between_patient_sd_prior: f64,
        known_sigma: f64,
        patient_count: u32,
        observations_per_patient: u32,
        replicates: u32,
        sampling: NutsSamplingSpec,
        minimum_rank_uniformity_p_value: f64,
        minimum_coverage: f64,
        maximum_coverage: f64,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if !(3..=32).contains(&patient_count)
            || !(2..=32).contains(&observations_per_patient)
            || !(20..=100).contains(&replicates)
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
                "hierarchical SBC inputs or calibration controls are invalid".into(),
            ));
        }
        let patients = (0..patient_count)
            .map(|index| HierarchicalPatientData {
                patient_id: format!("sbc-patient-{index:03}"),
                observations: vec![0.0; observations_per_patient as usize],
            })
            .collect();
        let template = GaussianHierarchyWorkerRequest::new(
            GaussianHierarchySpec {
                global_prior_mean,
                global_prior_sd,
                between_patient_sd_prior,
                known_sigma,
                patients,
            },
            sampling.clone(),
            environment_lock_sha256.clone(),
            worker_sha256.clone(),
            timeout_seconds,
        )?;
        let simulated_observations = u64::from(patient_count)
            .checked_mul(u64::from(observations_per_patient))
            .and_then(|value| value.checked_mul(u64::from(replicates)))
            .ok_or_else(|| BayesError::InvalidSpec("hierarchical SBC work overflow".into()))?;
        let maximum_simulated_observations = 100_000;
        if simulated_observations > maximum_simulated_observations {
            return Err(BayesError::InvalidSpec(
                "hierarchical SBC exceeds 100000 simulated observations".into(),
            ));
        }
        let per_fit_iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        let total_iterations = per_fit_iterations
            .checked_mul(u64::from(replicates))
            .ok_or_else(|| {
                BayesError::InvalidSpec("hierarchical SBC iterations overflow".into())
            })?;
        let maximum_total_iterations = 1_000_000;
        if total_iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "hierarchical SBC exceeds 1000000 total iterations".into(),
            ));
        }
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
            model: template.model,
            sampling,
            calibration: HierarchicalSbcCalibrationPolicy {
                replicates,
                patient_count,
                observations_per_patient,
                rank_bins: 10,
                interval_probability: 0.9,
                minimum_rank_uniformity_p_value,
                minimum_coverage,
                maximum_coverage,
            },
            diagnostic_policy: template.diagnostic_policy,
            resources: HierarchicalSbcResourceLimits {
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
pub struct HierarchicalSbcReplicate {
    pub replicate: u32,
    pub true_global_mean: f64,
    pub true_between_patient_sd: f64,
    pub global_mean_rank: u32,
    pub between_patient_sd_rank: u32,
    pub global_mean_covered: bool,
    pub between_patient_sd_covered: bool,
    pub r_hat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
}

impl SbcReplicateRow for HierarchicalSbcReplicate {
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
        ]
    }
    fn truths_valid(&self) -> bool {
        finite(&[self.true_global_mean, self.true_between_patient_sd])
            && self.true_between_patient_sd > 0.0
    }
    fn sampler(&self) -> SbcSamplerRow {
        SbcSamplerRow {
            r_hat: self.r_hat,
            ess_bulk: self.ess_bulk,
            ess_tail: self.ess_tail,
            minimum_ebfmi: None,
            divergences: self.divergences,
            max_tree_depth_hits: self.max_tree_depth_hits,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchicalSbcFailure {
    pub replicate: u32,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchicalSbcDiagnostics {
    pub global_mean: SbcParameterDiagnostics,
    pub between_patient_sd: SbcParameterDiagnostics,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroHierarchySbcWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub replicates: Vec<HierarchicalSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: HierarchicalSbcDiagnostics,
}

impl NumpyroHierarchySbcWorkerResult {
    pub fn validate(
        &self,
        request: &NumpyroHierarchySbcWorkerRequest,
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
                "hierarchical SBC result identity or dimensions mismatch".into(),
            ));
        }
        let posterior_draws = request.sampling.chains * request.sampling.draws_per_chain;
        validate_sbc_replicates(
            &self.replicates,
            &self.failures,
            &[
                &self.diagnostics.global_mean,
                &self.diagnostics.between_patient_sd,
            ],
            &request.calibration,
            posterior_draws,
            &request.diagnostic_policy,
            self.fit_state,
        )
        .map_err(|violation| violation.error("hierarchical"))
    }

    pub fn into_result(self, request: NumpyroHierarchySbcWorkerRequest) -> HierarchicalSbcResult {
        HierarchicalSbcResult {
            format: "marklab.bayesian_hierarchical_sbc",
            version: 1,
            backend: self.backend,
            jax_version: self.jax_version,
            model: request.model,
            sampling: request.sampling,
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
pub struct HierarchicalSbcResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub model: GaussianHierarchyModelIr,
    pub sampling: NutsSamplingSpec,
    pub calibration: HierarchicalSbcCalibrationPolicy,
    pub resources: HierarchicalSbcResourceLimits,
    pub fit_state: FitState,
    pub replicates: Vec<HierarchicalSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: HierarchicalSbcDiagnostics,
    pub claim_status: ClaimStatus,
    pub request_sha256: String,
}

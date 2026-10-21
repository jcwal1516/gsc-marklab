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
    BackendContract, BayesError, BetaBinomialGroupGenderRegressionModelIr,
    BetaBinomialGroupGenderRegressionWorkerRequest, FitState, HierarchicalSbcFailure,
    NutsSamplingSpec, SbcParameterDiagnostics, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_beta_binomial_group_gender_sbc_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_beta_binomial_group_gender_sbc_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderSbcResourceLimits {
    pub maximum_replicates: u32,
    pub maximum_simulated_trials: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroBetaBinomialGroupGenderSbcWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: BetaBinomialGroupGenderRegressionWorkerRequest,
    pub calibration: SbcCalibrationPolicy,
    pub resources: BetaBinomialGroupGenderSbcResourceLimits,
}

impl NumpyroBetaBinomialGroupGenderSbcWorkerRequest {
    pub fn new(
        source_request: BetaBinomialGroupGenderRegressionWorkerRequest,
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
                BayesError::InvalidSpec(
                    "beta-binomial group/gender SBC controls or identities are invalid".into(),
                )
            })?;
        let replicates = calibration.replicates;
        let trial_count = source_request
            .patients
            .iter()
            .map(|patient| patient.trials)
            .sum::<u64>();
        let simulated_trials = u64::from(replicates)
            .checked_mul(trial_count)
            .ok_or_else(|| {
                BayesError::InvalidSpec("beta-binomial group/gender SBC work overflow".into())
            })?;
        let maximum_simulated_trials = 200_000_000;
        let maximum_total_iterations = 1_000_000;
        let total_iterations = u64::from(replicates)
            .checked_mul(u64::from(source_request.sampling.chains))
            .and_then(|value| {
                value.checked_mul(u64::from(
                    source_request.sampling.tune_per_chain
                        + source_request.sampling.draws_per_chain,
                ))
            })
            .ok_or_else(|| {
                BayesError::InvalidSpec("beta-binomial group/gender SBC iterations overflow".into())
            })?;
        if simulated_trials > maximum_simulated_trials
            || total_iterations > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group/gender SBC exceeds trial or iteration limits".into(),
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
            resources: BetaBinomialGroupGenderSbcResourceLimits {
                maximum_replicates: 100,
                maximum_simulated_trials,
                maximum_total_iterations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderSbcReplicate {
    pub replicate: u32,
    pub true_intercept_log_odds: f64,
    pub true_group_log_odds_effect: f64,
    pub true_gender_log_odds_effect: f64,
    pub true_concentration: f64,
    pub true_marginal_probability_difference: f64,
    pub true_patient_probability_0: f64,
    pub intercept_log_odds_rank: u32,
    pub group_log_odds_effect_rank: u32,
    pub gender_log_odds_effect_rank: u32,
    pub concentration_rank: u32,
    pub marginal_probability_difference_rank: u32,
    pub patient_probability_0_rank: u32,
    pub intercept_log_odds_covered: bool,
    pub group_log_odds_effect_covered: bool,
    pub gender_log_odds_effect_covered: bool,
    pub concentration_covered: bool,
    pub marginal_probability_difference_covered: bool,
    pub patient_probability_0_covered: bool,
    pub r_hat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
    pub minimum_ebfmi: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
}

impl SbcReplicateRow for BetaBinomialGroupGenderSbcReplicate {
    fn replicate(&self) -> u32 {
        self.replicate
    }

    fn ranks(&self) -> Vec<(u32, bool)> {
        vec![
            (
                self.intercept_log_odds_rank,
                self.intercept_log_odds_covered,
            ),
            (
                self.group_log_odds_effect_rank,
                self.group_log_odds_effect_covered,
            ),
            (
                self.gender_log_odds_effect_rank,
                self.gender_log_odds_effect_covered,
            ),
            (self.concentration_rank, self.concentration_covered),
            (
                self.marginal_probability_difference_rank,
                self.marginal_probability_difference_covered,
            ),
            (
                self.patient_probability_0_rank,
                self.patient_probability_0_covered,
            ),
        ]
    }

    fn truths_valid(&self) -> bool {
        [
            self.true_intercept_log_odds,
            self.true_group_log_odds_effect,
            self.true_gender_log_odds_effect,
            self.true_concentration,
            self.true_marginal_probability_difference,
            self.true_patient_probability_0,
        ]
        .iter()
        .all(|value| value.is_finite())
            && self.true_concentration > 0.0
            && (-1.0..1.0).contains(&self.true_marginal_probability_difference)
            && (0.0..1.0).contains(&self.true_patient_probability_0)
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
pub struct BetaBinomialGroupGenderSbcDiagnostics {
    pub intercept_log_odds: SbcParameterDiagnostics,
    pub group_log_odds_effect: SbcParameterDiagnostics,
    pub gender_log_odds_effect: SbcParameterDiagnostics,
    pub concentration: SbcParameterDiagnostics,
    pub marginal_probability_difference: SbcParameterDiagnostics,
    pub patient_probability_0: SbcParameterDiagnostics,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroBetaBinomialGroupGenderSbcWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub replicates: Vec<BetaBinomialGroupGenderSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: BetaBinomialGroupGenderSbcDiagnostics,
}

impl NumpyroBetaBinomialGroupGenderSbcWorkerResult {
    pub fn validate(
        &self,
        request: &NumpyroBetaBinomialGroupGenderSbcWorkerRequest,
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
                "beta-binomial group/gender SBC result identity or dimensions mismatch".into(),
            ));
        }
        let posterior_draws = request.source_request.sampling.chains
            * request.source_request.sampling.draws_per_chain;
        validate_sbc_replicates(
            &self.replicates,
            &self.failures,
            &[
                &self.diagnostics.intercept_log_odds,
                &self.diagnostics.group_log_odds_effect,
                &self.diagnostics.gender_log_odds_effect,
                &self.diagnostics.concentration,
                &self.diagnostics.marginal_probability_difference,
                &self.diagnostics.patient_probability_0,
            ],
            &request.calibration,
            posterior_draws,
            &request.source_request.diagnostic_policy,
            self.fit_state,
        )
        .map_err(|violation| violation.error("beta-binomial group/gender"))
    }

    pub fn into_result(
        self,
        request: NumpyroBetaBinomialGroupGenderSbcWorkerRequest,
    ) -> BetaBinomialGroupGenderSbcResult {
        BetaBinomialGroupGenderSbcResult {
            format: "marklab.bayesian_beta_binomial_group_gender_sbc",
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
pub struct BetaBinomialGroupGenderSbcResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub model: BetaBinomialGroupGenderRegressionModelIr,
    pub sampling: NutsSamplingSpec,
    pub calibration: SbcCalibrationPolicy,
    pub resources: BetaBinomialGroupGenderSbcResourceLimits,
    pub fit_state: FitState,
    pub replicates: Vec<BetaBinomialGroupGenderSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: BetaBinomialGroupGenderSbcDiagnostics,
    pub claim_status: ClaimStatus,
    pub request_sha256: String,
}

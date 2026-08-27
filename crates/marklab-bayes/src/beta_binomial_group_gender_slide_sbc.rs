use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    BackendContract, BayesError, BetaBinomialGroupGenderSlideHierarchyModelIr,
    BetaBinomialGroupGenderSlideHierarchyWorkerRequest, FitState, HierarchicalSbcFailure,
    HierarchicalSbcParameterDiagnostics, NutsSamplingSpec, WorkerBackend,
};

const REQUEST_FORMAT: &str =
    "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_request";
const RESULT_FORMAT: &str =
    "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideSbcCalibrationPolicy {
    pub replicates: u32,
    pub rank_bins: u32,
    pub interval_probability: f64,
    pub maximum_tree_depth: u32,
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideSbcResourceLimits {
    pub maximum_replicates: u32,
    pub maximum_simulated_trials: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    pub calibration: BetaBinomialGroupGenderSlideSbcCalibrationPolicy,
    pub resources: BetaBinomialGroupGenderSlideSbcResourceLimits,
}

impl NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_request: BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
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
                "beta-binomial group/gender slide SBC controls or identities are invalid".into(),
            ));
        }
        let trial_count = source_request
            .slides
            .iter()
            .map(|slide| slide.trials)
            .sum::<u64>();
        let simulated_trials = u64::from(replicates)
            .checked_mul(trial_count)
            .ok_or_else(|| {
                BayesError::InvalidSpec("beta-binomial group/gender slide SBC work overflow".into())
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
                BayesError::InvalidSpec(
                    "beta-binomial group/gender slide SBC iterations overflow".into(),
                )
            })?;
        if simulated_trials > maximum_simulated_trials
            || total_iterations > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group/gender slide SBC exceeds trial or iteration limits".into(),
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
            calibration: BetaBinomialGroupGenderSlideSbcCalibrationPolicy {
                replicates,
                rank_bins: 10,
                interval_probability: 0.9,
                maximum_tree_depth: 12,
                minimum_rank_uniformity_p_value,
                minimum_coverage,
                maximum_coverage,
            },
            resources: BetaBinomialGroupGenderSlideSbcResourceLimits {
                maximum_replicates: 100,
                maximum_simulated_trials,
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
pub struct BetaBinomialGroupGenderSlideSbcReplicate {
    pub replicate: u32,
    pub true_intercept_log_odds: f64,
    pub true_group_log_odds_effect: f64,
    pub true_gender_log_odds_effect: f64,
    pub true_patient_log_odds_sd: f64,
    pub true_slide_concentration: f64,
    pub true_marginal_probability_difference: f64,
    pub true_patient_probability_0: f64,
    pub true_patient_random_effect_0: f64,
    pub intercept_log_odds_rank: u32,
    pub group_log_odds_effect_rank: u32,
    pub gender_log_odds_effect_rank: u32,
    pub patient_log_odds_sd_rank: u32,
    pub slide_concentration_rank: u32,
    pub marginal_probability_difference_rank: u32,
    pub patient_probability_0_rank: u32,
    pub patient_random_effect_0_rank: u32,
    pub intercept_log_odds_covered: bool,
    pub group_log_odds_effect_covered: bool,
    pub gender_log_odds_effect_covered: bool,
    pub patient_log_odds_sd_covered: bool,
    pub slide_concentration_covered: bool,
    pub marginal_probability_difference_covered: bool,
    pub patient_probability_0_covered: bool,
    pub patient_random_effect_0_covered: bool,
    pub r_hat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
    pub minimum_ebfmi: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderSlideSbcDiagnostics {
    pub intercept_log_odds: HierarchicalSbcParameterDiagnostics,
    pub group_log_odds_effect: HierarchicalSbcParameterDiagnostics,
    pub gender_log_odds_effect: HierarchicalSbcParameterDiagnostics,
    pub patient_log_odds_sd: HierarchicalSbcParameterDiagnostics,
    pub slide_concentration: HierarchicalSbcParameterDiagnostics,
    pub marginal_probability_difference: HierarchicalSbcParameterDiagnostics,
    pub patient_probability_0: HierarchicalSbcParameterDiagnostics,
    pub patient_random_effect_0: HierarchicalSbcParameterDiagnostics,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroBetaBinomialGroupGenderSlideSbcWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub replicates: Vec<BetaBinomialGroupGenderSlideSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: BetaBinomialGroupGenderSlideSbcDiagnostics,
}

impl NumpyroBetaBinomialGroupGenderSlideSbcWorkerResult {
    pub fn validate(
        &self,
        request: &NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest,
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
                "beta-binomial group/gender slide SBC result identity or dimensions mismatch"
                    .into(),
            ));
        }
        let posterior_draws = request.source_request.sampling.chains
            * request.source_request.sampling.draws_per_chain;
        let policy = &request.source_request.diagnostic_policy;
        let mut dispositions = BTreeSet::new();
        for row in &self.replicates {
            if row.replicate >= request.calibration.replicates
                || !dispositions.insert(row.replicate)
                || [
                    row.intercept_log_odds_rank,
                    row.group_log_odds_effect_rank,
                    row.gender_log_odds_effect_rank,
                    row.patient_log_odds_sd_rank,
                    row.slide_concentration_rank,
                    row.marginal_probability_difference_rank,
                    row.patient_probability_0_rank,
                    row.patient_random_effect_0_rank,
                ]
                .iter()
                .any(|rank| *rank > posterior_draws)
                || ![
                    row.true_intercept_log_odds,
                    row.true_group_log_odds_effect,
                    row.true_gender_log_odds_effect,
                    row.true_patient_log_odds_sd,
                    row.true_slide_concentration,
                    row.true_marginal_probability_difference,
                    row.true_patient_probability_0,
                    row.true_patient_random_effect_0,
                    row.r_hat,
                    row.ess_bulk,
                    row.ess_tail,
                    row.minimum_ebfmi,
                ]
                .iter()
                .all(|value| value.is_finite())
                || row.true_patient_log_odds_sd <= 0.0
                || row.true_slide_concentration <= 0.0
                || !(-1.0..1.0).contains(&row.true_marginal_probability_difference)
                || !(0.0..1.0).contains(&row.true_patient_probability_0)
                || row.r_hat > policy.maximum_r_hat
                || row.ess_bulk < policy.minimum_bulk_ess
                || row.ess_tail < policy.minimum_tail_ess
                || row.minimum_ebfmi < policy.minimum_ebfmi
                || row.divergences > policy.maximum_divergences
                || row.max_tree_depth_hits > policy.maximum_tree_depth_hits
            {
                return Err(BayesError::WorkerContract(
                    "beta-binomial group/gender slide SBC replicate is invalid".into(),
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
                    "beta-binomial group/gender slide SBC failure disposition is invalid".into(),
                ));
            }
        }
        let diagnostics = [
            &self.diagnostics.intercept_log_odds,
            &self.diagnostics.group_log_odds_effect,
            &self.diagnostics.gender_log_odds_effect,
            &self.diagnostics.patient_log_odds_sd,
            &self.diagnostics.slide_concentration,
            &self.diagnostics.marginal_probability_difference,
            &self.diagnostics.patient_probability_0,
            &self.diagnostics.patient_random_effect_0,
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
                    "beta-binomial group/gender slide SBC diagnostics are invalid".into(),
                ));
            }
        }
        let passes = self.failures.is_empty()
            && [
                &self.diagnostics.intercept_log_odds,
                &self.diagnostics.group_log_odds_effect,
                &self.diagnostics.gender_log_odds_effect,
                &self.diagnostics.patient_log_odds_sd,
                &self.diagnostics.slide_concentration,
                &self.diagnostics.marginal_probability_difference,
                &self.diagnostics.patient_probability_0,
                &self.diagnostics.patient_random_effect_0,
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
                "beta-binomial group/gender slide SBC disposition or fit state is invalid".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest,
    ) -> BetaBinomialGroupGenderSlideSbcResult {
        BetaBinomialGroupGenderSlideSbcResult {
            format: "marklab.bayesian_beta_binomial_group_gender_slide_hierarchy_sbc",
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
pub struct BetaBinomialGroupGenderSlideSbcResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub model: BetaBinomialGroupGenderSlideHierarchyModelIr,
    pub sampling: NutsSamplingSpec,
    pub calibration: BetaBinomialGroupGenderSlideSbcCalibrationPolicy,
    pub resources: BetaBinomialGroupGenderSlideSbcResourceLimits,
    pub fit_state: FitState,
    pub replicates: Vec<BetaBinomialGroupGenderSlideSbcReplicate>,
    pub failures: Vec<HierarchicalSbcFailure>,
    pub diagnostics: BetaBinomialGroupGenderSlideSbcDiagnostics,
    pub claim_status: &'static str,
    pub request_sha256: String,
}

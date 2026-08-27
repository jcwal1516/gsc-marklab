use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    model::{PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    sha256_hex, BackendContract, BayesError, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    NutsSamplingSpec, SamplingSummary, SarScalarSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupPatientData {
    pub patient_id: String,
    pub group: String,
    pub successes: u64,
    pub trials: u64,
}

#[derive(Clone, Debug)]
pub struct BetaBinomialGroupRegressionSpec {
    pub reference_group: String,
    pub comparison_group: String,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
    pub patients: Vec<BetaBinomialGroupPatientData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupRegressionModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub reference_group: String,
    pub comparison_group: String,
    pub intercept_prior: &'static str,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub group_effect_prior: &'static str,
    pub group_effect_prior_sd: f64,
    pub concentration_prior: &'static str,
    pub concentration_prior_sd: f64,
    pub patient_probability: &'static str,
    pub likelihood: &'static str,
    pub inference_parameterization: &'static str,
    pub biological_unit: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupRegressionResourceLimits {
    pub maximum_patients: u32,
    pub maximum_total_trials: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupRegressionWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: BetaBinomialGroupRegressionModelIr,
    pub patients: Vec<BetaBinomialGroupPatientData>,
    pub sampling: NutsSamplingSpec,
    pub resources: BetaBinomialGroupRegressionResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl BetaBinomialGroupRegressionWorkerRequest {
    pub fn new(
        spec: BetaBinomialGroupRegressionSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if spec.reference_group.is_empty()
            || spec.reference_group.trim() != spec.reference_group
            || spec.comparison_group.is_empty()
            || spec.comparison_group.trim() != spec.comparison_group
            || spec.reference_group == spec.comparison_group
            || !spec.intercept_prior_mean.is_finite()
            || [
                spec.intercept_prior_sd,
                spec.group_effect_prior_sd,
                spec.concentration_prior_sd,
            ]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
            || !(8..=512).contains(&spec.patients.len())
            || !is_sha256(&environment_lock_sha256)
            || !is_sha256(&worker_sha256)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group regression controls or identities are invalid".into(),
            ));
        }
        let mut patients = spec.patients;
        patients.sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut ids = BTreeSet::new();
        let mut group_counts = BTreeMap::new();
        let mut total_trials = 0_u64;
        for patient in &patients {
            if patient.patient_id.is_empty()
                || patient.patient_id.trim() != patient.patient_id
                || patient.patient_id.len() > 128
                || !ids.insert(patient.patient_id.as_str())
                || (patient.group != spec.reference_group && patient.group != spec.comparison_group)
                || patient.trials == 0
                || patient.successes > patient.trials
            {
                return Err(BayesError::InvalidSpec(
                    "beta-binomial group patient rows are invalid".into(),
                ));
            }
            *group_counts
                .entry(patient.group.as_str())
                .or_insert(0_usize) += 1;
            total_trials = total_trials.checked_add(patient.trials).ok_or_else(|| {
                BayesError::InvalidSpec("beta-binomial group trial count overflow".into())
            })?;
        }
        if group_counts
            .get(spec.reference_group.as_str())
            .copied()
            .unwrap_or(0)
            < 4
            || group_counts
                .get(spec.comparison_group.as_str())
                .copied()
                .unwrap_or(0)
                < 4
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group regression requires four patients per group".into(),
            ));
        }
        let maximum_total_trials = 10_000_000;
        if total_trials > maximum_total_trials {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group regression exceeds 10000000 total trials".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        if u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain)
            > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group regression exceeds 400000 NUTS iterations".into(),
            ));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "pymc",
                version: PYMC_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: BetaBinomialGroupRegressionModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "patient_beta_binomial_group_regression",
                reference_group: spec.reference_group,
                comparison_group: spec.comparison_group,
                intercept_prior: "normal_log_odds",
                intercept_prior_mean: spec.intercept_prior_mean,
                intercept_prior_sd: spec.intercept_prior_sd,
                group_effect_prior: "normal_log_odds_difference",
                group_effect_prior_sd: spec.group_effect_prior_sd,
                concentration_prior: "half_normal",
                concentration_prior_sd: spec.concentration_prior_sd,
                patient_probability: "beta_group_mean_concentration",
                likelihood: "beta_binomial_collapsed_patient_counts",
                inference_parameterization: "collapsed_with_exact_conditional_patient_draws",
                biological_unit: "patient",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            patients,
            sampling,
            resources: BetaBinomialGroupRegressionResourceLimits {
                maximum_patients: 512,
                maximum_total_trials,
                maximum_total_iterations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
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
pub struct BetaBinomialGroupRegressionPosterior {
    pub intercept_log_odds: SarScalarSummary,
    pub group_log_odds_effect: SarScalarSummary,
    pub reference_probability: SarScalarSummary,
    pub comparison_probability: SarScalarSummary,
    pub probability_difference_comparison_minus_reference: SarScalarSummary,
    pub odds_ratio: SarScalarSummary,
    pub concentration: SarScalarSummary,
    pub overdispersion_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupPatientPosterior {
    pub patient_id: String,
    pub group: String,
    pub successes: u64,
    pub trials: u64,
    pub observed_proportion: f64,
    pub posterior_probability: SarScalarSummary,
    pub shrinkage_toward_group: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupPosteriorPredictive {
    pub observed_reference_successes: u64,
    pub observed_comparison_successes: u64,
    pub replicated_reference_successes_mean: f64,
    pub replicated_comparison_successes_mean: f64,
    pub probability_replicated_reference_successes_at_least_observed: f64,
    pub probability_replicated_comparison_successes_at_least_observed: f64,
    pub observed_group_mean_proportion_difference: f64,
    pub replicated_group_mean_proportion_difference_mean: f64,
    pub probability_replicated_difference_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupRegressionWorkerResult {
    pub(crate) format: String,
    pub(crate) version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialGroupRegressionPosterior,
    pub patients: Vec<BetaBinomialGroupPatientPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialGroupPosteriorPredictive,
}

impl BetaBinomialGroupRegressionWorkerResult {
    pub fn validate(
        &self,
        request: &BetaBinomialGroupRegressionWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        self.validate_for_backend(
            request,
            request_sha256,
            "marklab.pymc_beta_binomial_group_regression_worker_result",
            &request.backend,
        )
    }

    pub(crate) fn validate_for_backend(
        &self,
        request: &BetaBinomialGroupRegressionWorkerRequest,
        request_sha256: &str,
        result_format: &str,
        backend: &BackendContract,
    ) -> Result<(), BayesError> {
        if self.format != result_format
            || self.version != 1
            || self.backend.name != backend.name
            || self.backend.version != backend.version
            || self.backend.python_version != backend.python_version
            || self.backend.environment_lock_sha256 != backend.environment_lock_sha256
            || self.backend.worker_sha256 != backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.patients.len() != request.patients.len()
        {
            return Err(BayesError::WorkerContract(
                "beta-binomial group result identity or dimensions mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.completed_draws != expected_draws
            || self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
        {
            return Err(BayesError::WorkerContract(
                "beta-binomial group sampling counts mismatch".into(),
            ));
        }
        validate_summary(&self.posterior.intercept_log_odds, SummarySupport::Real)?;
        validate_summary(&self.posterior.group_log_odds_effect, SummarySupport::Real)?;
        validate_summary(&self.posterior.reference_probability, SummarySupport::Unit)?;
        validate_summary(&self.posterior.comparison_probability, SummarySupport::Unit)?;
        validate_summary(
            &self
                .posterior
                .probability_difference_comparison_minus_reference,
            SummarySupport::Difference,
        )?;
        validate_summary(&self.posterior.odds_ratio, SummarySupport::Positive)?;
        validate_summary(&self.posterior.concentration, SummarySupport::Positive)?;
        if !(0.0..1.0).contains(&self.posterior.overdispersion_mean) {
            return Err(BayesError::WorkerContract(
                "beta-binomial group overdispersion is invalid".into(),
            ));
        }
        for (actual, expected) in self.patients.iter().zip(&request.patients) {
            validate_summary(&actual.posterior_probability, SummarySupport::Unit)?;
            let observed = expected.successes as f64 / expected.trials as f64;
            if actual.patient_id != expected.patient_id
                || actual.group != expected.group
                || actual.successes != expected.successes
                || actual.trials != expected.trials
                || (actual.observed_proportion - observed).abs() > 1e-12
                || !actual.shrinkage_toward_group.is_finite()
            {
                return Err(BayesError::WorkerContract(
                    "beta-binomial group patient summary is invalid".into(),
                ));
            }
        }
        validate_predictive(self, request)?;
        let diagnostics_pass = diagnostic_pass(&self.diagnostics, &request.diagnostic_policy);
        if (self.fit_state == FitState::Complete) != diagnostics_pass {
            return Err(BayesError::WorkerContract(
                "beta-binomial group fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: BetaBinomialGroupRegressionWorkerRequest,
        input: BetaBinomialGroupRegressionInputIdentity,
    ) -> BetaBinomialGroupRegressionResult {
        BetaBinomialGroupRegressionResult {
            format: "marklab.bayesian_beta_binomial_group_regression",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            sampling: self.sampling,
            fit_state: self.fit_state,
            posterior: self.posterior,
            patients: self.patients,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            claim_status: if self.fit_state == FitState::Complete {
                "experimental_patient_group_composition"
            } else {
                "diagnostic_only_nonconverged"
            },
            request_sha256: self.request_sha256,
        }
    }
}

#[derive(Clone, Copy)]
enum SummarySupport {
    Real,
    Unit,
    Difference,
    Positive,
}

fn validate_summary(summary: &SarScalarSummary, support: SummarySupport) -> Result<(), BayesError> {
    if ![
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]
    .iter()
    .all(|value| value.is_finite())
        || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
        || matches!(support, SummarySupport::Unit)
            && (!(0.0..=1.0).contains(&summary.mean)
                || !(0.0..=1.0).contains(&summary.interval_lower)
                || !(0.0..=1.0).contains(&summary.interval_upper))
        || matches!(support, SummarySupport::Difference)
            && (!(-1.0..=1.0).contains(&summary.mean)
                || !(-1.0..=1.0).contains(&summary.interval_lower)
                || !(-1.0..=1.0).contains(&summary.interval_upper))
        || matches!(support, SummarySupport::Positive)
            && (summary.mean <= 0.0 || summary.interval_lower < 0.0)
    {
        return Err(BayesError::WorkerContract(
            "beta-binomial group posterior summary is invalid".into(),
        ));
    }
    Ok(())
}

fn validate_predictive(
    result: &BetaBinomialGroupRegressionWorkerResult,
    request: &BetaBinomialGroupRegressionWorkerRequest,
) -> Result<(), BayesError> {
    let reference = request.model.reference_group.as_str();
    let comparison = request.model.comparison_group.as_str();
    let observed_reference: u64 = request
        .patients
        .iter()
        .filter(|patient| patient.group == reference)
        .map(|patient| patient.successes)
        .sum();
    let observed_comparison: u64 = request
        .patients
        .iter()
        .filter(|patient| patient.group == comparison)
        .map(|patient| patient.successes)
        .sum();
    let group_mean = |group: &str| {
        let values = request
            .patients
            .iter()
            .filter(|patient| patient.group == group)
            .map(|patient| patient.successes as f64 / patient.trials as f64)
            .collect::<Vec<_>>();
        values.iter().sum::<f64>() / values.len() as f64
    };
    let observed_difference = group_mean(comparison) - group_mean(reference);
    let predictive = &result.posterior_predictive;
    if predictive.observed_reference_successes != observed_reference
        || predictive.observed_comparison_successes != observed_comparison
        || (predictive.observed_group_mean_proportion_difference - observed_difference).abs()
            > 1e-12
        || ![
            predictive.replicated_reference_successes_mean,
            predictive.replicated_comparison_successes_mean,
            predictive.probability_replicated_reference_successes_at_least_observed,
            predictive.probability_replicated_comparison_successes_at_least_observed,
            predictive.replicated_group_mean_proportion_difference_mean,
            predictive.probability_replicated_difference_at_least_observed,
        ]
        .iter()
        .all(|value| value.is_finite())
        || ![
            predictive.probability_replicated_reference_successes_at_least_observed,
            predictive.probability_replicated_comparison_successes_at_least_observed,
            predictive.probability_replicated_difference_at_least_observed,
        ]
        .iter()
        .all(|value| (0.0..=1.0).contains(value))
    {
        return Err(BayesError::WorkerContract(
            "beta-binomial group posterior predictive summary is invalid".into(),
        ));
    }
    Ok(())
}

fn diagnostic_pass(diagnostics: &NormalMeanDiagnostics, policy: &DiagnosticPolicy) -> bool {
    diagnostics.prior_predictive_finite
        && diagnostics.posterior_finite
        && diagnostics.constraints_valid
        && diagnostics.identifiability_checks_passed
        && diagnostics.r_hat <= policy.maximum_r_hat
        && diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && diagnostics.ess_tail >= policy.minimum_tail_ess
        && diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && diagnostics.divergences <= policy.maximum_divergences
        && diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupRegressionInputIdentity {
    pub path: String,
    pub patient_data_sha256: String,
    pub reference_patients: usize,
    pub comparison_patients: usize,
}

pub fn beta_binomial_group_data_sha256(
    patients: &[BetaBinomialGroupPatientData],
) -> Result<String, BayesError> {
    Ok(sha256_hex(&serde_json::to_vec(patients)?))
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupRegressionResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: BetaBinomialGroupRegressionModelIr,
    pub input: BetaBinomialGroupRegressionInputIdentity,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: BetaBinomialGroupRegressionPosterior,
    pub patients: Vec<BetaBinomialGroupPatientPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialGroupPosteriorPredictive,
    pub seed: u64,
    pub claim_status: &'static str,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_group_without_four_patients() {
        let patients = (0..8)
            .map(|index| BetaBinomialGroupPatientData {
                patient_id: format!("p-{index}"),
                group: if index < 5 { "MSS" } else { "MSI" }.into(),
                successes: 5,
                trials: 10,
            })
            .collect();
        let result = BetaBinomialGroupRegressionWorkerRequest::new(
            BetaBinomialGroupRegressionSpec {
                reference_group: "MSS".into(),
                comparison_group: "MSI".into(),
                intercept_prior_mean: 0.0,
                intercept_prior_sd: 2.0,
                group_effect_prior_sd: 1.0,
                concentration_prior_sd: 20.0,
                patients,
            },
            NutsSamplingSpec {
                chains: 2,
                tune_per_chain: 100,
                draws_per_chain: 100,
                target_accept: 0.9,
                seed: 1,
            },
            "0".repeat(64),
            "1".repeat(64),
            60,
        );
        assert!(matches!(result, Err(BayesError::InvalidSpec(_))));
    }
}

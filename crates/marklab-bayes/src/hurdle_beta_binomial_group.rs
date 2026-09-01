use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    model::{PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    sha256_hex,
    validation::{diagnostics_satisfy_policy, is_lower_hex_sha256},
    BackendContract, BayesError, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    NutsSamplingSpec, SamplingSummary, SarScalarSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct HurdleBetaBinomialGroupPatientData {
    pub patient_id: String,
    pub group: String,
    pub successes: u64,
    pub trials: u64,
}

#[derive(Clone, Debug)]
pub struct HurdleBetaBinomialGroupSpec {
    pub reference_group: String,
    pub comparison_group: String,
    pub presence_intercept_prior_mean: f64,
    pub presence_intercept_prior_sd: f64,
    pub presence_group_effect_prior_sd: f64,
    pub abundance_intercept_prior_mean: f64,
    pub abundance_intercept_prior_sd: f64,
    pub abundance_group_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
    pub patients: Vec<HurdleBetaBinomialGroupPatientData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HurdleBetaBinomialGroupModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub reference_group: String,
    pub comparison_group: String,
    pub presence_intercept_prior: &'static str,
    pub presence_intercept_prior_mean: f64,
    pub presence_intercept_prior_sd: f64,
    pub presence_group_effect_prior: &'static str,
    pub presence_group_effect_prior_sd: f64,
    pub abundance_intercept_prior: &'static str,
    pub abundance_intercept_prior_mean: f64,
    pub abundance_intercept_prior_sd: f64,
    pub abundance_group_effect_prior: &'static str,
    pub abundance_group_effect_prior_sd: f64,
    pub concentration_prior: &'static str,
    pub concentration_prior_sd: f64,
    pub likelihood: &'static str,
    pub zero_process: &'static str,
    pub positive_process: &'static str,
    pub observation_unit: &'static str,
    pub biological_unit: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct HurdleBetaBinomialGroupResourceLimits {
    pub maximum_patients: u32,
    pub maximum_total_trials: u64,
    pub maximum_total_iterations: u64,
    pub maximum_predictive_quantile_evaluations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct HurdleBetaBinomialGroupWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: HurdleBetaBinomialGroupModelIr,
    pub patients: Vec<HurdleBetaBinomialGroupPatientData>,
    pub sampling: NutsSamplingSpec,
    pub resources: HurdleBetaBinomialGroupResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl HurdleBetaBinomialGroupWorkerRequest {
    pub fn new(
        spec: HurdleBetaBinomialGroupSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        let scales = [
            spec.presence_intercept_prior_sd,
            spec.presence_group_effect_prior_sd,
            spec.abundance_intercept_prior_sd,
            spec.abundance_group_effect_prior_sd,
            spec.concentration_prior_sd,
        ];
        if !valid_name(&spec.reference_group)
            || !valid_name(&spec.comparison_group)
            || spec.reference_group == spec.comparison_group
            || !spec.presence_intercept_prior_mean.is_finite()
            || !spec.abundance_intercept_prior_mean.is_finite()
            || scales
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            || !(8..=512).contains(&spec.patients.len())
            || !is_lower_hex_sha256(&environment_lock_sha256)
            || !is_lower_hex_sha256(&worker_sha256)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "hurdle beta-binomial controls or identities are invalid".into(),
            ));
        }
        let mut patients = spec.patients;
        patients.sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut patient_ids = BTreeSet::new();
        let mut group_counts = BTreeMap::<&str, usize>::new();
        let mut zero_counts = BTreeMap::<&str, usize>::new();
        let mut positive_counts = BTreeMap::<&str, usize>::new();
        let mut total_trials = 0_u64;
        for patient in &patients {
            if !valid_name(&patient.patient_id)
                || !patient_ids.insert(patient.patient_id.as_str())
                || (patient.group != spec.reference_group && patient.group != spec.comparison_group)
                || patient.trials == 0
                || patient.successes > patient.trials
            {
                return Err(BayesError::InvalidSpec(
                    "hurdle beta-binomial patient rows are invalid".into(),
                ));
            }
            *group_counts.entry(patient.group.as_str()).or_default() += 1;
            if patient.successes == 0 {
                *zero_counts.entry(patient.group.as_str()).or_default() += 1;
            } else {
                *positive_counts.entry(patient.group.as_str()).or_default() += 1;
            }
            total_trials = total_trials.checked_add(patient.trials).ok_or_else(|| {
                BayesError::InvalidSpec("hurdle beta-binomial total trial overflow".into())
            })?;
        }
        for group in [&spec.reference_group, &spec.comparison_group] {
            if group_counts.get(group.as_str()).copied().unwrap_or(0) < 4
                || zero_counts.get(group.as_str()).copied().unwrap_or(0) == 0
                || positive_counts.get(group.as_str()).copied().unwrap_or(0) == 0
            {
                return Err(BayesError::InvalidSpec(
                    "hurdle beta-binomial requires four patients plus zeros and positives in each group"
                        .into(),
                ));
            }
        }
        if total_trials > 10_000_000 {
            return Err(BayesError::InvalidSpec(
                "hurdle beta-binomial exceeds 10000000 total trials".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        if u64::from(sampling.chains)
            * (u64::from(sampling.tune_per_chain) + u64::from(sampling.draws_per_chain))
            > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "hurdle beta-binomial exceeds 400000 NUTS iterations".into(),
            ));
        }
        let maximum_predictive_quantile_evaluations = 5_000_000;
        if u64::from(sampling.chains) * u64::from(sampling.draws_per_chain) * patients.len() as u64
            > maximum_predictive_quantile_evaluations
        {
            return Err(BayesError::InvalidSpec(
                "hurdle beta-binomial exceeds 5000000 predictive quantile evaluations".into(),
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
            model: HurdleBetaBinomialGroupModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "patient_hurdle_beta_binomial_group_regression",
                reference_group: spec.reference_group,
                comparison_group: spec.comparison_group,
                presence_intercept_prior: "normal_log_odds",
                presence_intercept_prior_mean: spec.presence_intercept_prior_mean,
                presence_intercept_prior_sd: spec.presence_intercept_prior_sd,
                presence_group_effect_prior: "normal_log_odds_difference",
                presence_group_effect_prior_sd: spec.presence_group_effect_prior_sd,
                abundance_intercept_prior: "normal_log_odds",
                abundance_intercept_prior_mean: spec.abundance_intercept_prior_mean,
                abundance_intercept_prior_sd: spec.abundance_intercept_prior_sd,
                abundance_group_effect_prior: "normal_log_odds_difference",
                abundance_group_effect_prior_sd: spec.abundance_group_effect_prior_sd,
                concentration_prior: "half_normal",
                concentration_prior_sd: spec.concentration_prior_sd,
                likelihood: "bernoulli_presence_and_zero_truncated_beta_binomial_positive_count",
                zero_process: "structural_hurdle_absence",
                positive_process: "exposure_adjusted_positive_abundance",
                observation_unit: "one_complete_class_count_and_total_exposure_per_patient",
                biological_unit: "patient",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            patients,
            sampling,
            resources: HurdleBetaBinomialGroupResourceLimits {
                maximum_patients: 512,
                maximum_total_trials: 10_000_000,
                maximum_total_iterations,
                maximum_predictive_quantile_evaluations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HurdleBetaBinomialGroupPosterior {
    pub presence_intercept_log_odds: SarScalarSummary,
    pub presence_group_log_odds_effect: SarScalarSummary,
    pub reference_presence_probability: SarScalarSummary,
    pub comparison_presence_probability: SarScalarSummary,
    pub presence_probability_difference_comparison_minus_reference: SarScalarSummary,
    pub positive_abundance_intercept_log_odds: SarScalarSummary,
    pub positive_abundance_group_log_odds_effect: SarScalarSummary,
    pub reference_positive_abundance_probability: SarScalarSummary,
    pub comparison_positive_abundance_probability: SarScalarSummary,
    pub positive_abundance_difference_comparison_minus_reference: SarScalarSummary,
    pub reference_unconditional_expected_proportion: SarScalarSummary,
    pub comparison_unconditional_expected_proportion: SarScalarSummary,
    pub unconditional_expected_proportion_difference: SarScalarSummary,
    pub concentration: SarScalarSummary,
    pub overdispersion_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HurdleBetaBinomialGroupPosteriorPredictive {
    pub observed_reference_positive_patients: u64,
    pub observed_comparison_positive_patients: u64,
    pub replicated_reference_positive_patients_mean: f64,
    pub replicated_comparison_positive_patients_mean: f64,
    pub probability_replicated_reference_positive_patients_at_least_observed: f64,
    pub probability_replicated_comparison_positive_patients_at_least_observed: f64,
    pub observed_reference_total_successes: u64,
    pub observed_comparison_total_successes: u64,
    pub replicated_reference_total_successes_mean: f64,
    pub replicated_comparison_total_successes_mean: f64,
    pub probability_replicated_reference_total_successes_at_least_observed: f64,
    pub probability_replicated_comparison_total_successes_at_least_observed: f64,
    pub predictive_quantile_evaluations: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HurdleBetaBinomialGroupWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: HurdleBetaBinomialGroupPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: HurdleBetaBinomialGroupPosteriorPredictive,
}

impl HurdleBetaBinomialGroupWorkerResult {
    pub fn validate(
        &self,
        request: &HurdleBetaBinomialGroupWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.pymc_hurdle_beta_binomial_group_worker_result"
            || self.version != 1
            || self.backend.name != request.backend.name
            || self.backend.version != request.backend.version
            || self.backend.python_version != request.backend.python_version
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "hurdle beta-binomial result identity mismatch".into(),
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
                "hurdle beta-binomial sampling counts mismatch".into(),
            ));
        }
        let posterior = &self.posterior;
        for summary in [
            &posterior.presence_intercept_log_odds,
            &posterior.presence_group_log_odds_effect,
            &posterior.positive_abundance_intercept_log_odds,
            &posterior.positive_abundance_group_log_odds_effect,
        ] {
            validate_summary(summary, None)?;
        }
        for summary in [
            &posterior.reference_presence_probability,
            &posterior.comparison_presence_probability,
            &posterior.reference_positive_abundance_probability,
            &posterior.comparison_positive_abundance_probability,
            &posterior.reference_unconditional_expected_proportion,
            &posterior.comparison_unconditional_expected_proportion,
        ] {
            validate_summary(summary, Some((0.0, 1.0)))?;
        }
        for summary in [
            &posterior.presence_probability_difference_comparison_minus_reference,
            &posterior.positive_abundance_difference_comparison_minus_reference,
            &posterior.unconditional_expected_proportion_difference,
        ] {
            validate_summary(summary, Some((-1.0, 1.0)))?;
        }
        validate_summary(&posterior.concentration, Some((0.0, f64::INFINITY)))?;
        if posterior.concentration.mean <= 0.0
            || !(0.0..1.0).contains(&posterior.overdispersion_mean)
        {
            return Err(BayesError::WorkerContract(
                "hurdle beta-binomial positive parameter is invalid".into(),
            ));
        }
        let predictive = &self.posterior_predictive;
        let reference_positive = request
            .patients
            .iter()
            .filter(|patient| {
                patient.group == request.model.reference_group && patient.successes > 0
            })
            .count() as u64;
        let comparison_positive = request
            .patients
            .iter()
            .filter(|patient| {
                patient.group == request.model.comparison_group && patient.successes > 0
            })
            .count() as u64;
        let reference_successes = request
            .patients
            .iter()
            .filter(|patient| patient.group == request.model.reference_group)
            .map(|patient| patient.successes)
            .sum::<u64>();
        let comparison_successes = request
            .patients
            .iter()
            .filter(|patient| patient.group == request.model.comparison_group)
            .map(|patient| patient.successes)
            .sum::<u64>();
        if predictive.observed_reference_positive_patients != reference_positive
            || predictive.observed_comparison_positive_patients != comparison_positive
            || predictive.observed_reference_total_successes != reference_successes
            || predictive.observed_comparison_total_successes != comparison_successes
            || predictive.predictive_quantile_evaluations
                > request.resources.maximum_predictive_quantile_evaluations
        {
            return Err(BayesError::WorkerContract(
                "hurdle beta-binomial observed or rejection summary mismatch".into(),
            ));
        }
        for value in [
            predictive.replicated_reference_positive_patients_mean,
            predictive.replicated_comparison_positive_patients_mean,
            predictive.replicated_reference_total_successes_mean,
            predictive.replicated_comparison_total_successes_mean,
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(BayesError::WorkerContract(
                    "hurdle beta-binomial replicated mean is invalid".into(),
                ));
            }
        }
        for value in [
            predictive.probability_replicated_reference_positive_patients_at_least_observed,
            predictive.probability_replicated_comparison_positive_patients_at_least_observed,
            predictive.probability_replicated_reference_total_successes_at_least_observed,
            predictive.probability_replicated_comparison_total_successes_at_least_observed,
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(BayesError::WorkerContract(
                    "hurdle beta-binomial predictive probability is invalid".into(),
                ));
            }
        }
        let diagnostics_pass =
            diagnostics_satisfy_policy(&self.diagnostics, &request.diagnostic_policy);
        if (self.fit_state == FitState::Complete) != diagnostics_pass {
            return Err(BayesError::WorkerContract(
                "hurdle beta-binomial fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: HurdleBetaBinomialGroupWorkerRequest,
        input: HurdleBetaBinomialGroupInputIdentity,
    ) -> HurdleBetaBinomialGroupResult {
        HurdleBetaBinomialGroupResult {
            format: "marklab.bayesian_hurdle_beta_binomial_group",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            sampling: self.sampling,
            fit_state: self.fit_state,
            posterior: self.posterior,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            claim_status: if self.fit_state == FitState::Complete {
                "experimental_patient_hurdle_group_association"
            } else {
                "diagnostic_only_nonconverged"
            },
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_summary(
    summary: &SarScalarSummary,
    support: Option<(f64, f64)>,
) -> Result<(), BayesError> {
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
        || support.is_some_and(|(lower, upper)| {
            summary.mean < lower
                || summary.mean > upper
                || summary.interval_lower < lower
                || summary.interval_upper > upper
        })
    {
        return Err(BayesError::WorkerContract(
            "hurdle beta-binomial posterior summary is invalid".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub struct HurdleBetaBinomialGroupInputIdentity {
    pub path: String,
    pub patient_data_sha256: String,
    pub patient_count: usize,
    pub zero_count: usize,
    pub positive_count: usize,
    pub reference_patients: usize,
    pub comparison_patients: usize,
    pub total_trials: u64,
}

pub fn hurdle_beta_binomial_group_data_sha256(
    patients: &[HurdleBetaBinomialGroupPatientData],
) -> Result<String, BayesError> {
    Ok(sha256_hex(&serde_json::to_vec(patients)?))
}

#[derive(Debug, Serialize)]
pub struct HurdleBetaBinomialGroupResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: HurdleBetaBinomialGroupModelIr,
    pub input: HurdleBetaBinomialGroupInputIdentity,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: HurdleBetaBinomialGroupPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: HurdleBetaBinomialGroupPosteriorPredictive,
    pub seed: u64,
    pub claim_status: &'static str,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_group_without_observed_zeros() {
        let patients = (0..8)
            .map(|index| HurdleBetaBinomialGroupPatientData {
                patient_id: format!("p-{index}"),
                group: if index < 4 { "MSS" } else { "MSI" }.into(),
                successes: if index == 0 { 0 } else { 2 },
                trials: 10,
            })
            .collect();
        assert!(matches!(
            HurdleBetaBinomialGroupWorkerRequest::new(
                HurdleBetaBinomialGroupSpec {
                    reference_group: "MSS".into(),
                    comparison_group: "MSI".into(),
                    presence_intercept_prior_mean: 0.0,
                    presence_intercept_prior_sd: 2.0,
                    presence_group_effect_prior_sd: 2.0,
                    abundance_intercept_prior_mean: -2.0,
                    abundance_intercept_prior_sd: 2.0,
                    abundance_group_effect_prior_sd: 2.0,
                    concentration_prior_sd: 50.0,
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
            ),
            Err(BayesError::InvalidSpec(_))
        ));
    }
}

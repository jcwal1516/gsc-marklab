use crate::is_worker_timeout;
use crate::validation::{
    diagnostics_satisfy_policy, is_exact_name, is_sha256,
    validate_mcse_summary as validate_supported_summary, SummarySupport,
};

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::backend_versions::PYTHON_VERSION;
use crate::ClaimStatus;
use crate::{
    model::{PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    BackendContract, BayesError, DiagnosticPolicy, FitState, McseScalarSummary, NutsSamplingSpec,
    PosteriorDiagnostics, SamplingSummary, WorkerBackend,
};
use marklab_core::backend::BackendCapability;
use marklab_core::backend::ModelMaturity;

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderPatientData {
    pub patient_id: String,
    pub group: String,
    pub gender: String,
    pub successes: u64,
    pub trials: u64,
}

/// Collapsed beta-binomial patient counts with additive group and gender effects.
///
/// Comparison indicators `g_i` and `s_i` give
/// `logit(p_i) = alpha + beta_group * g_i + beta_gender * s_i`. Counts have a
/// beta-binomial likelihood with `trials_i` trials and shapes `p_i * phi` and
/// `(1 - p_i) * phi`. Independent priors are
/// `alpha ~ Normal(intercept_prior_mean, intercept_prior_sd)`,
/// `beta_group ~ Normal(0, group_effect_prior_sd)`,
/// `beta_gender ~ Normal(0, gender_effect_prior_sd)`, and
/// `phi ~ HalfNormal(concentration_prior_sd)`. Normal scales are SDs; the
/// half-normal scale is its underlying normal SD. There is no interaction term.
/// Patient probabilities are integrated out and later drawn conditionally.
#[derive(Clone, Debug)]
pub struct BetaBinomialGroupGenderRegressionSpec {
    pub reference_group: String,
    pub comparison_group: String,
    pub reference_gender: String,
    pub comparison_gender: String,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub gender_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
    pub patients: Vec<BetaBinomialGroupGenderPatientData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderRegressionModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub reference_group: String,
    pub comparison_group: String,
    pub reference_gender: String,
    pub comparison_gender: String,
    pub intercept_prior: &'static str,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub group_effect_prior: &'static str,
    pub group_effect_prior_sd: f64,
    pub gender_effect_prior: &'static str,
    pub gender_effect_prior_sd: f64,
    pub concentration_prior: &'static str,
    pub concentration_prior_sd: f64,
    pub patient_probability: &'static str,
    pub likelihood: &'static str,
    pub interaction: &'static str,
    pub standardization: &'static str,
    pub biological_unit: &'static str,
    pub backend_capability: BackendCapability,
    pub maturity: ModelMaturity,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderRegressionResourceLimits {
    pub maximum_patients: u32,
    pub maximum_total_trials: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderRegressionWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: BetaBinomialGroupGenderRegressionModelIr,
    pub patients: Vec<BetaBinomialGroupGenderPatientData>,
    pub sampling: NutsSamplingSpec,
    pub resources: BetaBinomialGroupGenderRegressionResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl BetaBinomialGroupGenderRegressionWorkerRequest {
    pub fn new(
        spec: BetaBinomialGroupGenderRegressionSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if spec.reference_group.is_empty()
            || spec.comparison_group.is_empty()
            || spec.reference_group == spec.comparison_group
            || spec.reference_gender.is_empty()
            || spec.comparison_gender.is_empty()
            || spec.reference_gender == spec.comparison_gender
            || !spec.intercept_prior_mean.is_finite()
            || [
                spec.intercept_prior_sd,
                spec.group_effect_prior_sd,
                spec.gender_effect_prior_sd,
                spec.concentration_prior_sd,
            ]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
            || !is_sha256(&environment_lock_sha256)
            || !is_sha256(&worker_sha256)
            || !is_worker_timeout(timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group/gender regression controls are invalid".into(),
            ));
        }
        if !(16..=512).contains(&spec.patients.len()) {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group/gender regression requires 16 to 512 patients".into(),
            ));
        }
        let mut patients = spec.patients;
        patients.sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut identities = BTreeSet::new();
        let mut support = BTreeMap::<(String, String), usize>::new();
        let mut total_trials = 0_u64;
        for patient in &patients {
            if !is_exact_name(&patient.patient_id)
                || !identities.insert(patient.patient_id.as_str())
                || ![
                    spec.reference_group.as_str(),
                    spec.comparison_group.as_str(),
                ]
                .contains(&patient.group.as_str())
                || ![
                    spec.reference_gender.as_str(),
                    spec.comparison_gender.as_str(),
                ]
                .contains(&patient.gender.as_str())
                || patient.trials == 0
                || patient.successes > patient.trials
            {
                return Err(BayesError::InvalidSpec(
                    "beta-binomial group/gender patient row is invalid".into(),
                ));
            }
            *support
                .entry((patient.group.clone(), patient.gender.clone()))
                .or_default() += 1;
            total_trials = total_trials.checked_add(patient.trials).ok_or_else(|| {
                BayesError::InvalidSpec("beta-binomial group/gender trials overflow".into())
            })?;
        }
        for group in [&spec.reference_group, &spec.comparison_group] {
            for gender in [&spec.reference_gender, &spec.comparison_gender] {
                if support
                    .get(&(group.clone(), gender.clone()))
                    .copied()
                    .unwrap_or(0)
                    < 4
                {
                    return Err(BayesError::InvalidSpec(
                        "beta-binomial group/gender regression requires four patients per design cell"
                            .into(),
                    ));
                }
            }
        }
        let maximum_total_trials = 10_000_000;
        if total_trials > maximum_total_trials {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group/gender regression exceeds 10000000 total trials".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        if u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain)
            > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group/gender regression exceeds 400000 NUTS iterations".into(),
            ));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "pymc",
                version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                environment_lock_sha256,
                worker_sha256,
            },
            model: BetaBinomialGroupGenderRegressionModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "patient_beta_binomial_group_gender_regression",
                reference_group: spec.reference_group,
                comparison_group: spec.comparison_group,
                reference_gender: spec.reference_gender,
                comparison_gender: spec.comparison_gender,
                intercept_prior: "normal_log_odds",
                intercept_prior_mean: spec.intercept_prior_mean,
                intercept_prior_sd: spec.intercept_prior_sd,
                group_effect_prior: "normal_log_odds_difference",
                group_effect_prior_sd: spec.group_effect_prior_sd,
                gender_effect_prior: "normal_log_odds_difference",
                gender_effect_prior_sd: spec.gender_effect_prior_sd,
                concentration_prior: "half_normal",
                concentration_prior_sd: spec.concentration_prior_sd,
                patient_probability: "beta_additive_group_gender_mean_concentration",
                likelihood: "beta_binomial_collapsed_patient_counts",
                interaction: "none",
                standardization: "observed_gender_distribution",
                biological_unit: "patient",
                backend_capability: BackendCapability::Nuts,
                maturity: ModelMaturity::Experimental,
            },
            patients,
            sampling,
            resources: BetaBinomialGroupGenderRegressionResourceLimits {
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderRegressionPosterior {
    pub intercept_log_odds: McseScalarSummary,
    pub group_log_odds_effect: McseScalarSummary,
    pub gender_log_odds_effect: McseScalarSummary,
    pub reference_group_reference_gender_probability: McseScalarSummary,
    pub comparison_group_reference_gender_probability: McseScalarSummary,
    pub reference_group_comparison_gender_probability: McseScalarSummary,
    pub comparison_group_comparison_gender_probability: McseScalarSummary,
    pub reference_gender_probability_difference: McseScalarSummary,
    pub comparison_gender_probability_difference: McseScalarSummary,
    pub marginal_reference_group_probability: McseScalarSummary,
    pub marginal_comparison_group_probability: McseScalarSummary,
    pub marginal_probability_difference_comparison_minus_reference: McseScalarSummary,
    pub group_odds_ratio: McseScalarSummary,
    pub gender_odds_ratio: McseScalarSummary,
    pub concentration: McseScalarSummary,
    pub overdispersion_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderPatientPosterior {
    pub patient_id: String,
    pub group: String,
    pub gender: String,
    pub successes: u64,
    pub trials: u64,
    pub observed_proportion: f64,
    pub posterior_probability: McseScalarSummary,
    pub shrinkage_toward_linear_predictor: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderPosteriorPredictive {
    pub observed_total_successes: u64,
    pub replicated_total_successes_mean: f64,
    pub probability_replicated_total_successes_at_least_observed: f64,
    pub observed_group_mean_proportion_difference: f64,
    pub replicated_group_mean_proportion_difference_mean: f64,
    pub probability_replicated_group_difference_at_least_observed: f64,
    pub observed_gender_mean_proportion_difference: f64,
    pub replicated_gender_mean_proportion_difference_mean: f64,
    pub probability_replicated_gender_difference_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderRegressionWorkerResult {
    pub(crate) format: String,
    pub(crate) version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialGroupGenderRegressionPosterior,
    pub patients: Vec<BetaBinomialGroupGenderPatientPosterior>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: BetaBinomialGroupGenderPosteriorPredictive,
}

impl BetaBinomialGroupGenderRegressionWorkerResult {
    pub fn validate(
        &self,
        request: &BetaBinomialGroupGenderRegressionWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        self.validate_for_backend(
            request,
            request_sha256,
            "marklab.pymc_beta_binomial_group_gender_regression_worker_result",
            &request.backend,
        )
    }

    pub(crate) fn validate_for_backend(
        &self,
        request: &BetaBinomialGroupGenderRegressionWorkerRequest,
        request_sha256: &str,
        result_format: &str,
        backend: &BackendContract,
    ) -> Result<(), BayesError> {
        if self.format != result_format
            || self.version != 1
            || !self.backend.matches(backend)
            || self.request_sha256 != request_sha256
            || self.patients.len() != request.patients.len()
        {
            return Err(BayesError::WorkerContract(
                "beta-binomial group/gender result identity or dimensions mismatch".into(),
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
                "beta-binomial group/gender sampling counts mismatch".into(),
            ));
        }
        let posterior = &self.posterior;
        for summary in [
            &posterior.intercept_log_odds,
            &posterior.group_log_odds_effect,
            &posterior.gender_log_odds_effect,
        ] {
            validate_summary(summary, SummarySupport::Real)?;
        }
        for summary in [
            &posterior.reference_group_reference_gender_probability,
            &posterior.comparison_group_reference_gender_probability,
            &posterior.reference_group_comparison_gender_probability,
            &posterior.comparison_group_comparison_gender_probability,
            &posterior.marginal_reference_group_probability,
            &posterior.marginal_comparison_group_probability,
        ] {
            validate_summary(summary, SummarySupport::Unit)?;
        }
        for summary in [
            &posterior.reference_gender_probability_difference,
            &posterior.comparison_gender_probability_difference,
            &posterior.marginal_probability_difference_comparison_minus_reference,
        ] {
            validate_summary(summary, SummarySupport::Difference)?;
        }
        for summary in [
            &posterior.group_odds_ratio,
            &posterior.gender_odds_ratio,
            &posterior.concentration,
        ] {
            validate_summary(summary, SummarySupport::Positive)?;
        }
        if !(0.0..1.0).contains(&posterior.overdispersion_mean) {
            return Err(BayesError::WorkerContract(
                "beta-binomial group/gender overdispersion is invalid".into(),
            ));
        }
        for (actual, expected) in self.patients.iter().zip(&request.patients) {
            validate_summary(&actual.posterior_probability, SummarySupport::Unit)?;
            let observed = expected.successes as f64 / expected.trials as f64;
            if actual.patient_id != expected.patient_id
                || actual.group != expected.group
                || actual.gender != expected.gender
                || actual.successes != expected.successes
                || actual.trials != expected.trials
                || (actual.observed_proportion - observed).abs() > 1e-12
                || !actual.shrinkage_toward_linear_predictor.is_finite()
            {
                return Err(BayesError::WorkerContract(
                    "beta-binomial group/gender patient summary is invalid".into(),
                ));
            }
        }
        validate_predictive(self, request)?;
        let complete = diagnostics_satisfy_policy(&self.diagnostics, &request.diagnostic_policy);
        if (self.fit_state == FitState::Complete) != complete {
            return Err(BayesError::WorkerContract(
                "beta-binomial group/gender fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: BetaBinomialGroupGenderRegressionWorkerRequest,
        input: BetaBinomialGroupGenderRegressionInputIdentity,
    ) -> BetaBinomialGroupGenderRegressionResult {
        BetaBinomialGroupGenderRegressionResult {
            format: "marklab.bayesian_beta_binomial_group_gender_regression",
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
                ClaimStatus::ExperimentalPatientGroupCompositionAdjustedForGender
            } else {
                ClaimStatus::DiagnosticOnlyNonconverged
            },
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_summary(
    summary: &McseScalarSummary,
    support: SummarySupport,
) -> Result<(), BayesError> {
    validate_supported_summary(
        summary,
        support,
        "beta-binomial group/gender posterior summary is invalid",
    )
}

fn validate_predictive(
    result: &BetaBinomialGroupGenderRegressionWorkerResult,
    request: &BetaBinomialGroupGenderRegressionWorkerRequest,
) -> Result<(), BayesError> {
    let mean_difference = |field: fn(&BetaBinomialGroupGenderPatientData) -> &str,
                           comparison: &str,
                           reference: &str| {
        let mean = |level: &str| {
            let values = request
                .patients
                .iter()
                .filter(|patient| field(patient) == level)
                .map(|patient| patient.successes as f64 / patient.trials as f64)
                .collect::<Vec<_>>();
            values.iter().sum::<f64>() / values.len() as f64
        };
        mean(comparison) - mean(reference)
    };
    let group = mean_difference(
        |patient| patient.group.as_str(),
        request.model.comparison_group.as_str(),
        request.model.reference_group.as_str(),
    );
    let gender = mean_difference(
        |patient| patient.gender.as_str(),
        request.model.comparison_gender.as_str(),
        request.model.reference_gender.as_str(),
    );
    let predictive = &result.posterior_predictive;
    let observed_total: u64 = request
        .patients
        .iter()
        .map(|patient| patient.successes)
        .sum();
    if predictive.observed_total_successes != observed_total
        || (predictive.observed_group_mean_proportion_difference - group).abs() > 1e-12
        || (predictive.observed_gender_mean_proportion_difference - gender).abs() > 1e-12
        || ![
            predictive.replicated_total_successes_mean,
            predictive.probability_replicated_total_successes_at_least_observed,
            predictive.replicated_group_mean_proportion_difference_mean,
            predictive.probability_replicated_group_difference_at_least_observed,
            predictive.replicated_gender_mean_proportion_difference_mean,
            predictive.probability_replicated_gender_difference_at_least_observed,
        ]
        .iter()
        .all(|value| value.is_finite())
        || ![
            predictive.probability_replicated_total_successes_at_least_observed,
            predictive.probability_replicated_group_difference_at_least_observed,
            predictive.probability_replicated_gender_difference_at_least_observed,
        ]
        .iter()
        .all(|value| (0.0..=1.0).contains(value))
    {
        return Err(BayesError::WorkerContract(
            "beta-binomial group/gender posterior predictive summary is invalid".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderRegressionInputIdentity {
    pub path: String,
    pub patient_data_sha256: String,
    pub design_cell_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupGenderRegressionResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: BetaBinomialGroupGenderRegressionModelIr,
    pub input: BetaBinomialGroupGenderRegressionInputIdentity,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: BetaBinomialGroupGenderRegressionPosterior,
    pub patients: Vec<BetaBinomialGroupGenderPatientPosterior>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: BetaBinomialGroupGenderPosteriorPredictive,
    pub seed: u64,
    pub claim_status: ClaimStatus,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_design_cell_without_four_patients() {
        let patients = (0..16)
            .map(|index| BetaBinomialGroupGenderPatientData {
                patient_id: format!("p-{index:02}"),
                group: if index < 9 { "MSS" } else { "MSI" }.into(),
                gender: match index {
                    0..=4 | 9..=12 => "Male",
                    _ => "Female",
                }
                .into(),
                successes: 5,
                trials: 10,
            })
            .collect();
        let result = BetaBinomialGroupGenderRegressionWorkerRequest::new(
            BetaBinomialGroupGenderRegressionSpec {
                reference_group: "MSS".into(),
                comparison_group: "MSI".into(),
                reference_gender: "Male".into(),
                comparison_gender: "Female".into(),
                intercept_prior_mean: 0.0,
                intercept_prior_sd: 2.0,
                group_effect_prior_sd: 1.0,
                gender_effect_prior_sd: 1.0,
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
            "a".repeat(64),
            "b".repeat(64),
            30,
        );
        assert!(matches!(result, Err(BayesError::InvalidSpec(_))));
    }
}

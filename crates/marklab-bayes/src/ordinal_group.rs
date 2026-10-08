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
pub struct OrdinalGroupPatientData {
    pub patient_id: String,
    pub group: String,
    pub outcome_code: u32,
}

#[derive(Clone, Debug)]
pub struct OrdinalGroupSpec {
    pub reference_group: String,
    pub comparison_group: String,
    pub ordered_levels: Vec<String>,
    pub cutpoint_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub patients: Vec<OrdinalGroupPatientData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OrdinalGroupModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub reference_group: String,
    pub comparison_group: String,
    pub ordered_levels: Vec<String>,
    pub cutpoint_prior: &'static str,
    pub cutpoint_prior_sd: f64,
    pub group_effect_prior: &'static str,
    pub group_effect_prior_sd: f64,
    pub intercept: &'static str,
    pub likelihood: &'static str,
    pub proportional_odds_assumption: bool,
    pub observation_unit: &'static str,
    pub biological_unit: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct OrdinalGroupResourceLimits {
    pub maximum_patients: u32,
    pub maximum_levels: u32,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct OrdinalGroupWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: OrdinalGroupModelIr,
    pub patients: Vec<OrdinalGroupPatientData>,
    pub sampling: NutsSamplingSpec,
    pub resources: OrdinalGroupResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl OrdinalGroupWorkerRequest {
    pub fn new(
        spec: OrdinalGroupSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if !valid_name(&spec.reference_group)
            || !valid_name(&spec.comparison_group)
            || spec.reference_group == spec.comparison_group
            || !(2..=8).contains(&spec.ordered_levels.len())
            || !spec.cutpoint_prior_sd.is_finite()
            || spec.cutpoint_prior_sd <= 0.0
            || !spec.group_effect_prior_sd.is_finite()
            || spec.group_effect_prior_sd <= 0.0
            || !(8..=512).contains(&spec.patients.len())
            || !is_lower_hex_sha256(&environment_lock_sha256)
            || !is_lower_hex_sha256(&worker_sha256)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "ordinal-group controls or identities are invalid".into(),
            ));
        }
        if spec.ordered_levels.iter().any(|level| !valid_name(level))
            || spec.ordered_levels.iter().collect::<BTreeSet<_>>().len()
                != spec.ordered_levels.len()
        {
            return Err(BayesError::InvalidSpec(
                "ordinal-group ordered levels are invalid".into(),
            ));
        }
        let mut patients = spec.patients;
        patients.sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut patient_ids = BTreeSet::new();
        let mut group_counts = BTreeMap::new();
        let mut level_counts = vec![0_usize; spec.ordered_levels.len()];
        for patient in &patients {
            if !valid_name(&patient.patient_id)
                || !patient_ids.insert(patient.patient_id.as_str())
                || (patient.group != spec.reference_group && patient.group != spec.comparison_group)
                || patient.outcome_code as usize >= spec.ordered_levels.len()
            {
                return Err(BayesError::InvalidSpec(
                    "ordinal-group patient rows are invalid".into(),
                ));
            }
            *group_counts
                .entry(patient.group.as_str())
                .or_insert(0_usize) += 1;
            level_counts[patient.outcome_code as usize] += 1;
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
            || level_counts.contains(&0)
        {
            return Err(BayesError::InvalidSpec(
                "ordinal-group requires four patients per group and every level observed".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        if u64::from(sampling.chains)
            * (u64::from(sampling.tune_per_chain) + u64::from(sampling.draws_per_chain))
            > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "ordinal-group exceeds 400000 NUTS iterations".into(),
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
            model: OrdinalGroupModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "patient_proportional_odds_group_regression",
                reference_group: spec.reference_group,
                comparison_group: spec.comparison_group,
                ordered_levels: spec.ordered_levels,
                cutpoint_prior: "ordered_normal",
                cutpoint_prior_sd: spec.cutpoint_prior_sd,
                group_effect_prior: "normal_log_odds_difference",
                group_effect_prior_sd: spec.group_effect_prior_sd,
                intercept: "none_cutpoints_own_baseline_location",
                likelihood: "ordered_logistic_patient_outcome",
                proportional_odds_assumption: true,
                observation_unit: "one_complete_ordered_outcome_per_patient",
                biological_unit: "patient",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            patients,
            sampling,
            resources: OrdinalGroupResourceLimits {
                maximum_patients: 512,
                maximum_levels: 8,
                maximum_total_iterations,
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
pub struct OrdinalCutpointPosterior {
    pub lower_level: String,
    pub upper_level: String,
    #[serde(flatten)]
    pub summary: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinalLevelPosterior {
    pub level: String,
    pub reference_probability: SarScalarSummary,
    pub comparison_probability: SarScalarSummary,
    pub difference_comparison_minus_reference: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinalGroupPosterior {
    pub group_log_odds_effect: SarScalarSummary,
    pub cutpoints: Vec<OrdinalCutpointPosterior>,
    pub levels: Vec<OrdinalLevelPosterior>,
    pub reference_expected_code: SarScalarSummary,
    pub comparison_expected_code: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinalLevelPredictive {
    pub level: String,
    pub observed_reference_proportion: f64,
    pub observed_comparison_proportion: f64,
    pub replicated_reference_proportion_mean: f64,
    pub replicated_comparison_proportion_mean: f64,
    pub probability_absolute_replication_error_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinalGroupPosteriorPredictive {
    pub levels: Vec<OrdinalLevelPredictive>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinalGroupWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: OrdinalGroupPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: OrdinalGroupPosteriorPredictive,
}

impl OrdinalGroupWorkerResult {
    pub fn validate(
        &self,
        request: &OrdinalGroupWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        let levels = &request.model.ordered_levels;
        if self.format != "marklab.pymc_ordinal_group_worker_result"
            || self.version != 1
            || self.backend.name != request.backend.name
            || self.backend.version != request.backend.version
            || self.backend.python_version != request.backend.python_version
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.posterior.cutpoints.len() + 1 != levels.len()
            || self.posterior.levels.len() != levels.len()
            || self.posterior_predictive.levels.len() != levels.len()
        {
            return Err(BayesError::WorkerContract(
                "ordinal-group result identity or dimensions mismatch".into(),
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
                "ordinal-group sampling counts mismatch".into(),
            ));
        }
        validate_summary(&self.posterior.group_log_odds_effect, None)?;
        validate_summary(
            &self.posterior.reference_expected_code,
            Some((0.0, (levels.len() - 1) as f64)),
        )?;
        validate_summary(
            &self.posterior.comparison_expected_code,
            Some((0.0, (levels.len() - 1) as f64)),
        )?;
        let mut previous = f64::NEG_INFINITY;
        for (index, cutpoint) in self.posterior.cutpoints.iter().enumerate() {
            if cutpoint.lower_level != levels[index]
                || cutpoint.upper_level != levels[index + 1]
                || cutpoint.summary.mean <= previous
            {
                return Err(BayesError::WorkerContract(
                    "ordinal-group cutpoint order mismatch".into(),
                ));
            }
            validate_summary(&cutpoint.summary, None)?;
            previous = cutpoint.summary.mean;
        }
        let reference_count = request
            .patients
            .iter()
            .filter(|patient| patient.group == request.model.reference_group)
            .count() as f64;
        let comparison_count = request.patients.len() as f64 - reference_count;
        let mut reference_sum = 0.0;
        let mut comparison_sum = 0.0;
        for (index, ((posterior, predictive), level)) in self
            .posterior
            .levels
            .iter()
            .zip(&self.posterior_predictive.levels)
            .zip(levels)
            .enumerate()
        {
            if posterior.level != *level || predictive.level != *level {
                return Err(BayesError::WorkerContract(
                    "ordinal-group level order mismatch".into(),
                ));
            }
            validate_summary(&posterior.reference_probability, Some((0.0, 1.0)))?;
            validate_summary(&posterior.comparison_probability, Some((0.0, 1.0)))?;
            validate_summary(
                &posterior.difference_comparison_minus_reference,
                Some((-1.0, 1.0)),
            )?;
            reference_sum += posterior.reference_probability.mean;
            comparison_sum += posterior.comparison_probability.mean;
            let observed_reference = request
                .patients
                .iter()
                .filter(|patient| {
                    patient.group == request.model.reference_group
                        && patient.outcome_code as usize == index
                })
                .count() as f64
                / reference_count;
            let observed_comparison = request
                .patients
                .iter()
                .filter(|patient| {
                    patient.group == request.model.comparison_group
                        && patient.outcome_code as usize == index
                })
                .count() as f64
                / comparison_count;
            if (predictive.observed_reference_proportion - observed_reference).abs() > 1e-12
                || (predictive.observed_comparison_proportion - observed_comparison).abs() > 1e-12
                || [
                    predictive.replicated_reference_proportion_mean,
                    predictive.replicated_comparison_proportion_mean,
                    predictive.probability_absolute_replication_error_at_least_observed,
                ]
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            {
                return Err(BayesError::WorkerContract(
                    "ordinal-group predictive summaries are invalid".into(),
                ));
            }
        }
        if (reference_sum - 1.0).abs() > 1e-6 || (comparison_sum - 1.0).abs() > 1e-6 {
            return Err(BayesError::WorkerContract(
                "ordinal-group category probabilities do not sum to one".into(),
            ));
        }
        let diagnostics_pass =
            diagnostics_satisfy_policy(&self.diagnostics, &request.diagnostic_policy);
        if (self.fit_state == FitState::Complete) != diagnostics_pass {
            return Err(BayesError::WorkerContract(
                "ordinal-group fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: OrdinalGroupWorkerRequest,
        input: OrdinalGroupInputIdentity,
    ) -> OrdinalGroupResult {
        OrdinalGroupResult {
            format: "marklab.bayesian_ordinal_group",
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
                "experimental_patient_ordinal_group_association"
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
            "ordinal-group posterior summary is invalid".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub struct OrdinalGroupInputIdentity {
    pub path: String,
    pub patient_data_sha256: String,
    pub patient_count: usize,
    pub level_count: usize,
    pub reference_patients: usize,
    pub comparison_patients: usize,
}

pub fn ordinal_group_data_sha256(
    patients: &[OrdinalGroupPatientData],
) -> Result<String, BayesError> {
    Ok(sha256_hex(&serde_json::to_vec(patients)?))
}

#[derive(Debug, Serialize)]
pub struct OrdinalGroupResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: OrdinalGroupModelIr,
    pub input: OrdinalGroupInputIdentity,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: OrdinalGroupPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: OrdinalGroupPosteriorPredictive,
    pub seed: u64,
    pub claim_status: &'static str,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_unobserved_ordered_level() {
        let patients = (0..8)
            .map(|index| OrdinalGroupPatientData {
                patient_id: format!("p-{index}"),
                group: if index < 4 { "MSS" } else { "MSI" }.into(),
                outcome_code: (index % 2) as u32,
            })
            .collect();
        assert!(matches!(
            OrdinalGroupWorkerRequest::new(
                OrdinalGroupSpec {
                    reference_group: "MSS".into(),
                    comparison_group: "MSI".into(),
                    ordered_levels: vec!["I".into(), "II".into(), "III".into()],
                    cutpoint_prior_sd: 2.0,
                    group_effect_prior_sd: 2.0,
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

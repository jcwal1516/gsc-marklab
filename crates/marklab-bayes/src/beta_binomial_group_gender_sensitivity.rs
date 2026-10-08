use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::{
    BayesError, BetaBinomialGroupGenderPatientPosterior,
    BetaBinomialGroupGenderPosteriorPredictive, BetaBinomialGroupGenderRegressionInputIdentity,
    BetaBinomialGroupGenderRegressionPosterior, BetaBinomialGroupGenderRegressionWorkerResult,
    FitState, NormalMeanDiagnostics, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct BetaBinomialGroupGenderSensitivityScenario {
    pub scenario_id: String,
    pub intercept_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub gender_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
}

#[derive(Debug)]
pub struct BetaBinomialGroupGenderSensitivityScenarioRun {
    pub scenario: BetaBinomialGroupGenderSensitivityScenario,
    pub result: BetaBinomialGroupGenderRegressionWorkerResult,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupGenderSensitivityScenarioResult {
    pub scenario_id: String,
    pub intercept_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub gender_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialGroupGenderRegressionPosterior,
    pub patients: Vec<BetaBinomialGroupGenderPatientPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialGroupGenderPosteriorPredictive,
    pub parameter_standardized_shifts: BTreeMap<String, f64>,
    pub patient_probability_rms_standardized_shift: f64,
    pub maximum_absolute_standardized_shift: f64,
    pub material_shift: bool,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupGenderSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub input: BetaBinomialGroupGenderRegressionInputIdentity,
    pub base_scenario: &'static str,
    pub material_standardized_shift: f64,
    pub fit_state: FitState,
    pub sensitivity_status: &'static str,
    pub scenarios: Vec<BetaBinomialGroupGenderSensitivityScenarioResult>,
    pub material_shift_scenarios: Vec<String>,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl BetaBinomialGroupGenderSensitivityResult {
    pub fn new(
        input: BetaBinomialGroupGenderRegressionInputIdentity,
        seed: u64,
        material_standardized_shift: f64,
        runs: Vec<BetaBinomialGroupGenderSensitivityScenarioRun>,
    ) -> Result<Self, BayesError> {
        if !material_standardized_shift.is_finite()
            || material_standardized_shift <= 0.0
            || runs.len() != 9
            || runs[0].scenario.scenario_id != "baseline"
        {
            return Err(BayesError::InvalidSpec(
                "group/gender sensitivity controls are invalid".into(),
            ));
        }
        let mut names = BTreeSet::new();
        for run in &runs {
            if run.scenario.scenario_id.is_empty()
                || !names.insert(run.scenario.scenario_id.as_str())
                || [
                    run.scenario.intercept_prior_sd,
                    run.scenario.group_effect_prior_sd,
                    run.scenario.gender_effect_prior_sd,
                    run.scenario.concentration_prior_sd,
                ]
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            {
                return Err(BayesError::InvalidSpec(
                    "group/gender sensitivity scenarios are invalid".into(),
                ));
            }
        }
        let baseline = &runs[0].result;
        let baseline_parameters = parameter_summaries(&baseline.posterior);
        if baseline_parameters
            .values()
            .any(|(_, sd)| !sd.is_finite() || *sd <= 0.0)
            || baseline
                .patients
                .iter()
                .any(|patient| patient.posterior_probability.sd <= 0.0)
        {
            return Err(BayesError::WorkerContract(
                "group/gender baseline cannot scale sensitivity".into(),
            ));
        }
        let baseline_patients = baseline
            .patients
            .iter()
            .map(|patient| {
                (
                    patient.patient_id.clone(),
                    patient.group.clone(),
                    patient.gender.clone(),
                    patient.posterior_probability.mean,
                    patient.posterior_probability.sd,
                )
            })
            .collect::<Vec<_>>();
        let all_complete = runs
            .iter()
            .all(|run| run.result.fit_state == FitState::Complete);
        let mut material_shift_scenarios = Vec::new();
        let mut scenarios = Vec::with_capacity(runs.len());
        for run in runs {
            let current = parameter_summaries(&run.result.posterior);
            if current.len() != baseline_parameters.len()
                || run.result.patients.len() != baseline_patients.len()
            {
                return Err(BayesError::WorkerContract(
                    "group/gender sensitivity dimensions differ".into(),
                ));
            }
            let mut shifts = BTreeMap::new();
            for (name, (baseline_mean, baseline_sd)) in &baseline_parameters {
                let (mean, _) = current.get(name).ok_or_else(|| {
                    BayesError::WorkerContract(
                        "group/gender sensitivity parameter identities differ".into(),
                    )
                })?;
                shifts.insert(name.clone(), (mean - baseline_mean) / baseline_sd);
            }
            let mut patient_squared = 0.0;
            for (patient, (id, group, gender, mean, sd)) in
                run.result.patients.iter().zip(&baseline_patients)
            {
                if patient.patient_id != id.as_str()
                    || patient.group != group.as_str()
                    || patient.gender != gender.as_str()
                {
                    return Err(BayesError::WorkerContract(
                        "group/gender sensitivity patient identities differ".into(),
                    ));
                }
                patient_squared += ((patient.posterior_probability.mean - mean) / sd).powi(2);
            }
            let patient_shift = (patient_squared / baseline_patients.len() as f64).sqrt();
            let maximum = shifts
                .values()
                .map(|value| value.abs())
                .fold(patient_shift, f64::max);
            let material = run.result.fit_state == FitState::Complete
                && maximum >= material_standardized_shift;
            if material {
                material_shift_scenarios.push(run.scenario.scenario_id.clone());
            }
            scenarios.push(BetaBinomialGroupGenderSensitivityScenarioResult {
                scenario_id: run.scenario.scenario_id,
                intercept_prior_sd: run.scenario.intercept_prior_sd,
                group_effect_prior_sd: run.scenario.group_effect_prior_sd,
                gender_effect_prior_sd: run.scenario.gender_effect_prior_sd,
                concentration_prior_sd: run.scenario.concentration_prior_sd,
                backend: run.result.backend,
                request_sha256: run.result.request_sha256,
                fit_state: run.result.fit_state,
                sampling: run.result.sampling,
                posterior: run.result.posterior,
                patients: run.result.patients,
                diagnostics: run.result.diagnostics,
                posterior_predictive: run.result.posterior_predictive,
                parameter_standardized_shifts: shifts,
                patient_probability_rms_standardized_shift: patient_shift,
                maximum_absolute_standardized_shift: maximum,
                material_shift: material,
            });
        }
        let sensitivity_status = if !all_complete {
            "diagnostic_only_nonconverged"
        } else if material_shift_scenarios.is_empty() {
            "stable_within_declared_grid"
        } else {
            "sensitive_within_declared_grid"
        };
        Ok(Self {
            format: "marklab.bayesian_beta_binomial_group_gender_sensitivity",
            version: 1,
            input,
            base_scenario: "baseline",
            material_standardized_shift,
            fit_state: if all_complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            sensitivity_status,
            scenarios,
            material_shift_scenarios,
            seed,
            claim_status: if all_complete {
                "experimental_prior_sensitivity"
            } else {
                "diagnostic_only_prior_sensitivity"
            },
        })
    }
}

fn parameter_summaries(
    posterior: &BetaBinomialGroupGenderRegressionPosterior,
) -> BTreeMap<String, (f64, f64)> {
    [
        ("intercept_log_odds", &posterior.intercept_log_odds),
        ("group_log_odds_effect", &posterior.group_log_odds_effect),
        ("gender_log_odds_effect", &posterior.gender_log_odds_effect),
        (
            "reference_group_reference_gender_probability",
            &posterior.reference_group_reference_gender_probability,
        ),
        (
            "comparison_group_reference_gender_probability",
            &posterior.comparison_group_reference_gender_probability,
        ),
        (
            "reference_group_comparison_gender_probability",
            &posterior.reference_group_comparison_gender_probability,
        ),
        (
            "comparison_group_comparison_gender_probability",
            &posterior.comparison_group_comparison_gender_probability,
        ),
        (
            "reference_gender_probability_difference",
            &posterior.reference_gender_probability_difference,
        ),
        (
            "comparison_gender_probability_difference",
            &posterior.comparison_gender_probability_difference,
        ),
        (
            "marginal_reference_group_probability",
            &posterior.marginal_reference_group_probability,
        ),
        (
            "marginal_comparison_group_probability",
            &posterior.marginal_comparison_group_probability,
        ),
        (
            "marginal_probability_difference_comparison_minus_reference",
            &posterior.marginal_probability_difference_comparison_minus_reference,
        ),
        ("group_odds_ratio", &posterior.group_odds_ratio),
        ("gender_odds_ratio", &posterior.gender_odds_ratio),
        ("concentration", &posterior.concentration),
    ]
    .into_iter()
    .map(|(name, summary)| (name.into(), (summary.mean, summary.sd)))
    .collect()
}

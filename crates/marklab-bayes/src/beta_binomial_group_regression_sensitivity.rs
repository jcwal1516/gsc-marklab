use std::collections::BTreeSet;

use serde::Serialize;

use crate::{
    BayesError, BetaBinomialGroupPatientPosterior, BetaBinomialGroupPosteriorPredictive,
    BetaBinomialGroupRegressionInputIdentity, BetaBinomialGroupRegressionPosterior,
    BetaBinomialGroupRegressionWorkerResult, FitState, NormalMeanDiagnostics, SamplingSummary,
    WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct BetaBinomialGroupSensitivityScenario {
    pub scenario_id: String,
    pub intercept_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
}

#[derive(Debug)]
pub struct BetaBinomialGroupSensitivityScenarioRun {
    pub scenario: BetaBinomialGroupSensitivityScenario,
    pub result: BetaBinomialGroupRegressionWorkerResult,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupSensitivityScenarioResult {
    pub scenario_id: String,
    pub intercept_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialGroupRegressionPosterior,
    pub patients: Vec<BetaBinomialGroupPatientPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialGroupPosteriorPredictive,
    pub intercept_standardized_shift: f64,
    pub group_effect_standardized_shift: f64,
    pub reference_probability_standardized_shift: f64,
    pub comparison_probability_standardized_shift: f64,
    pub probability_difference_standardized_shift: f64,
    pub odds_ratio_standardized_shift: f64,
    pub concentration_standardized_shift: f64,
    pub patient_probability_rms_standardized_shift: f64,
    pub maximum_absolute_standardized_shift: f64,
    pub material_shift: bool,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub input: BetaBinomialGroupRegressionInputIdentity,
    pub base_scenario: &'static str,
    pub material_standardized_shift: f64,
    pub fit_state: FitState,
    pub sensitivity_status: &'static str,
    pub scenarios: Vec<BetaBinomialGroupSensitivityScenarioResult>,
    pub material_shift_scenarios: Vec<String>,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl BetaBinomialGroupSensitivityResult {
    pub fn new(
        input: BetaBinomialGroupRegressionInputIdentity,
        seed: u64,
        material_standardized_shift: f64,
        runs: Vec<BetaBinomialGroupSensitivityScenarioRun>,
    ) -> Result<Self, BayesError> {
        if !material_standardized_shift.is_finite()
            || material_standardized_shift <= 0.0
            || runs.len() != 7
            || runs[0].scenario.scenario_id != "baseline"
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group sensitivity controls are invalid".into(),
            ));
        }
        let mut names = BTreeSet::new();
        for run in &runs {
            if run.scenario.scenario_id.is_empty()
                || !names.insert(run.scenario.scenario_id.as_str())
                || [
                    run.scenario.intercept_prior_sd,
                    run.scenario.group_effect_prior_sd,
                    run.scenario.concentration_prior_sd,
                ]
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            {
                return Err(BayesError::InvalidSpec(
                    "beta-binomial group sensitivity scenarios are invalid".into(),
                ));
            }
        }
        let baseline = &runs[0].result;
        let scales = [
            baseline.posterior.intercept_log_odds.sd,
            baseline.posterior.group_log_odds_effect.sd,
            baseline.posterior.reference_probability.sd,
            baseline.posterior.comparison_probability.sd,
            baseline
                .posterior
                .probability_difference_comparison_minus_reference
                .sd,
            baseline.posterior.odds_ratio.sd,
            baseline.posterior.concentration.sd,
        ];
        if scales
            .iter()
            .any(|scale| !scale.is_finite() || *scale <= 0.0)
            || baseline
                .patients
                .iter()
                .any(|patient| patient.posterior_probability.sd <= 0.0)
        {
            return Err(BayesError::WorkerContract(
                "beta-binomial group baseline cannot scale sensitivity".into(),
            ));
        }
        let baseline_means = [
            baseline.posterior.intercept_log_odds.mean,
            baseline.posterior.group_log_odds_effect.mean,
            baseline.posterior.reference_probability.mean,
            baseline.posterior.comparison_probability.mean,
            baseline
                .posterior
                .probability_difference_comparison_minus_reference
                .mean,
            baseline.posterior.odds_ratio.mean,
            baseline.posterior.concentration.mean,
        ];
        let baseline_patients = baseline
            .patients
            .iter()
            .map(|patient| {
                (
                    patient.patient_id.clone(),
                    patient.group.clone(),
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
            if run.result.patients.len() != baseline_patients.len() {
                return Err(BayesError::WorkerContract(
                    "beta-binomial group sensitivity patient dimensions differ".into(),
                ));
            }
            let means = [
                run.result.posterior.intercept_log_odds.mean,
                run.result.posterior.group_log_odds_effect.mean,
                run.result.posterior.reference_probability.mean,
                run.result.posterior.comparison_probability.mean,
                run.result
                    .posterior
                    .probability_difference_comparison_minus_reference
                    .mean,
                run.result.posterior.odds_ratio.mean,
                run.result.posterior.concentration.mean,
            ];
            let shifts: [f64; 7] =
                std::array::from_fn(|index| (means[index] - baseline_means[index]) / scales[index]);
            let mut patient_squared = 0.0;
            for (patient, (baseline_id, baseline_group, baseline_mean, baseline_sd)) in
                run.result.patients.iter().zip(&baseline_patients)
            {
                if patient.patient_id != baseline_id.as_str()
                    || patient.group != baseline_group.as_str()
                {
                    return Err(BayesError::WorkerContract(
                        "beta-binomial group sensitivity patient identities differ".into(),
                    ));
                }
                patient_squared +=
                    ((patient.posterior_probability.mean - baseline_mean) / baseline_sd).powi(2);
            }
            let patient_shift = (patient_squared / baseline_patients.len() as f64).sqrt();
            let maximum = shifts
                .iter()
                .map(|shift| shift.abs())
                .fold(patient_shift, f64::max);
            let material = run.result.fit_state == FitState::Complete
                && maximum >= material_standardized_shift;
            if material {
                material_shift_scenarios.push(run.scenario.scenario_id.clone());
            }
            scenarios.push(BetaBinomialGroupSensitivityScenarioResult {
                scenario_id: run.scenario.scenario_id,
                intercept_prior_sd: run.scenario.intercept_prior_sd,
                group_effect_prior_sd: run.scenario.group_effect_prior_sd,
                concentration_prior_sd: run.scenario.concentration_prior_sd,
                backend: run.result.backend,
                request_sha256: run.result.request_sha256,
                fit_state: run.result.fit_state,
                sampling: run.result.sampling,
                posterior: run.result.posterior,
                patients: run.result.patients,
                diagnostics: run.result.diagnostics,
                posterior_predictive: run.result.posterior_predictive,
                intercept_standardized_shift: shifts[0],
                group_effect_standardized_shift: shifts[1],
                reference_probability_standardized_shift: shifts[2],
                comparison_probability_standardized_shift: shifts[3],
                probability_difference_standardized_shift: shifts[4],
                odds_ratio_standardized_shift: shifts[5],
                concentration_standardized_shift: shifts[6],
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
            format: "marklab.bayesian_beta_binomial_group_regression_sensitivity",
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

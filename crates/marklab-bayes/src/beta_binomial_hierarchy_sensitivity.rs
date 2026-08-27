use std::collections::BTreeSet;

use serde::Serialize;

use crate::{
    BayesError, BetaBinomialHierarchyInputIdentity, BetaBinomialHierarchyPosterior,
    BetaBinomialHierarchyWorkerResult, BetaBinomialPatientPosterior,
    BetaBinomialPosteriorPredictive, FitState, NormalMeanDiagnostics, SamplingSummary,
    WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct BetaBinomialSensitivityScenario {
    pub scenario_id: String,
    pub population_alpha: f64,
    pub population_beta: f64,
    pub concentration_prior_sd: f64,
}

#[derive(Debug)]
pub struct BetaBinomialSensitivityScenarioRun {
    pub scenario: BetaBinomialSensitivityScenario,
    pub result: BetaBinomialHierarchyWorkerResult,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialSensitivityScenarioResult {
    pub scenario_id: String,
    pub population_alpha: f64,
    pub population_beta: f64,
    pub concentration_prior_sd: f64,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialHierarchyPosterior,
    pub patients: Vec<BetaBinomialPatientPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialPosteriorPredictive,
    pub population_probability_standardized_shift: f64,
    pub concentration_standardized_shift: f64,
    pub patient_probability_rms_standardized_shift: f64,
    pub maximum_absolute_standardized_shift: f64,
    pub material_shift: bool,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub input: BetaBinomialHierarchyInputIdentity,
    pub base_scenario: &'static str,
    pub material_standardized_shift: f64,
    pub fit_state: FitState,
    pub sensitivity_status: &'static str,
    pub scenarios: Vec<BetaBinomialSensitivityScenarioResult>,
    pub material_shift_scenarios: Vec<String>,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl BetaBinomialSensitivityResult {
    pub fn new(
        input: BetaBinomialHierarchyInputIdentity,
        seed: u64,
        material_standardized_shift: f64,
        runs: Vec<BetaBinomialSensitivityScenarioRun>,
    ) -> Result<Self, BayesError> {
        if !material_standardized_shift.is_finite()
            || material_standardized_shift <= 0.0
            || runs.len() != 7
            || runs[0].scenario.scenario_id != "baseline"
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial sensitivity controls are invalid".into(),
            ));
        }
        let mut names = BTreeSet::new();
        for run in &runs {
            if run.scenario.scenario_id.is_empty()
                || !names.insert(run.scenario.scenario_id.as_str())
                || [
                    run.scenario.population_alpha,
                    run.scenario.population_beta,
                    run.scenario.concentration_prior_sd,
                ]
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            {
                return Err(BayesError::InvalidSpec(
                    "beta-binomial sensitivity scenarios are invalid".into(),
                ));
            }
        }
        let baseline = &runs[0].result;
        let population_scale = baseline.posterior.population_probability.sd;
        let concentration_scale = baseline.posterior.concentration.sd;
        if !population_scale.is_finite()
            || population_scale <= 0.0
            || !concentration_scale.is_finite()
            || concentration_scale <= 0.0
            || baseline
                .patients
                .iter()
                .any(|patient| patient.posterior_probability.sd <= 0.0)
        {
            return Err(BayesError::WorkerContract(
                "beta-binomial baseline cannot scale sensitivity".into(),
            ));
        }
        let baseline_population = baseline.posterior.population_probability.mean;
        let baseline_concentration = baseline.posterior.concentration.mean;
        let baseline_patients = baseline
            .patients
            .iter()
            .map(|patient| {
                (
                    patient.patient_id.clone(),
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
                    "beta-binomial sensitivity patient dimensions differ".into(),
                ));
            }
            let population_shift = (run.result.posterior.population_probability.mean
                - baseline_population)
                / population_scale;
            let concentration_shift = (run.result.posterior.concentration.mean
                - baseline_concentration)
                / concentration_scale;
            let mut patient_squared = 0.0;
            for (patient, (baseline_id, baseline_mean, baseline_sd)) in
                run.result.patients.iter().zip(&baseline_patients)
            {
                if patient.patient_id != baseline_id.as_str() {
                    return Err(BayesError::WorkerContract(
                        "beta-binomial sensitivity patient identities differ".into(),
                    ));
                }
                patient_squared +=
                    ((patient.posterior_probability.mean - baseline_mean) / baseline_sd).powi(2);
            }
            let patient_shift = (patient_squared / baseline_patients.len() as f64).sqrt();
            let maximum = population_shift
                .abs()
                .max(concentration_shift.abs())
                .max(patient_shift);
            let material = run.result.fit_state == FitState::Complete
                && maximum >= material_standardized_shift;
            if material {
                material_shift_scenarios.push(run.scenario.scenario_id.clone());
            }
            scenarios.push(BetaBinomialSensitivityScenarioResult {
                scenario_id: run.scenario.scenario_id,
                population_alpha: run.scenario.population_alpha,
                population_beta: run.scenario.population_beta,
                concentration_prior_sd: run.scenario.concentration_prior_sd,
                backend: run.result.backend,
                request_sha256: run.result.request_sha256,
                fit_state: run.result.fit_state,
                sampling: run.result.sampling,
                posterior: run.result.posterior,
                patients: run.result.patients,
                diagnostics: run.result.diagnostics,
                posterior_predictive: run.result.posterior_predictive,
                population_probability_standardized_shift: population_shift,
                concentration_standardized_shift: concentration_shift,
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
            format: "marklab.bayesian_beta_binomial_hierarchy_sensitivity",
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

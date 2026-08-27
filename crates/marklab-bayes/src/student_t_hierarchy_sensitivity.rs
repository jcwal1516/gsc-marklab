use std::collections::BTreeSet;

use serde::Serialize;

use crate::{BayesError, FitState};
use crate::{
    GaussianHierarchyInputIdentity, NormalMeanDiagnostics, PartialPoolingSummary, SamplingSummary,
    StudentTHierarchyPosterior, StudentTHierarchyPosteriorPredictive,
    StudentTHierarchyWorkerResult, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct StudentTSensitivityScenario {
    pub scenario_id: String,
    pub global_prior_sd: f64,
    pub between_patient_sd_prior: f64,
    pub observation_sd_prior: f64,
    pub degrees_of_freedom_excess_rate: f64,
}

#[derive(Debug)]
pub struct StudentTSensitivityScenarioRun {
    pub scenario: StudentTSensitivityScenario,
    pub result: StudentTHierarchyWorkerResult,
}

#[derive(Debug, Serialize)]
pub struct StudentTSensitivityScenarioResult {
    pub scenario_id: String,
    pub global_prior_sd: f64,
    pub between_patient_sd_prior: f64,
    pub observation_sd_prior: f64,
    pub degrees_of_freedom_excess_rate: f64,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: StudentTHierarchyPosterior,
    pub partial_pooling: Vec<PartialPoolingSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: StudentTHierarchyPosteriorPredictive,
    pub global_mean_shift_standardized: f64,
    pub between_patient_sd_shift_standardized: f64,
    pub observation_sd_shift_standardized: f64,
    pub degrees_of_freedom_shift_standardized: f64,
    pub maximum_absolute_standardized_shift: f64,
    pub material_shift: bool,
}

#[derive(Debug, Serialize)]
pub struct StudentTHierarchySensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub input: GaussianHierarchyInputIdentity,
    pub base_scenario: &'static str,
    pub material_standardized_shift: f64,
    pub fit_state: FitState,
    pub sensitivity_state: &'static str,
    pub scenarios: Vec<StudentTSensitivityScenarioResult>,
    pub material_shift_scenarios: Vec<String>,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl StudentTHierarchySensitivityResult {
    pub fn new(
        input: GaussianHierarchyInputIdentity,
        seed: u64,
        material_standardized_shift: f64,
        runs: Vec<StudentTSensitivityScenarioRun>,
    ) -> Result<Self, BayesError> {
        if !material_standardized_shift.is_finite()
            || material_standardized_shift <= 0.0
            || runs.len() != 9
            || runs[0].scenario.scenario_id != "baseline"
        {
            return Err(BayesError::InvalidSpec(
                "Student-t sensitivity controls are invalid".into(),
            ));
        }
        let mut names = BTreeSet::new();
        for run in &runs {
            if run.scenario.scenario_id.is_empty()
                || !names.insert(run.scenario.scenario_id.as_str())
                || [
                    run.scenario.global_prior_sd,
                    run.scenario.between_patient_sd_prior,
                    run.scenario.observation_sd_prior,
                    run.scenario.degrees_of_freedom_excess_rate,
                ]
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            {
                return Err(BayesError::InvalidSpec(
                    "Student-t sensitivity scenarios are invalid".into(),
                ));
            }
        }
        let baseline = &runs[0].result.posterior;
        let scales = [
            (baseline.global_mean.mean, baseline.global_mean.sd),
            (
                baseline.between_patient_sd.mean,
                baseline.between_patient_sd.sd,
            ),
            (baseline.observation_sd.mean, baseline.observation_sd.sd),
            (
                baseline.degrees_of_freedom.mean,
                baseline.degrees_of_freedom.sd,
            ),
        ];
        if scales.iter().any(|(_, sd)| !sd.is_finite() || *sd <= 0.0) {
            return Err(BayesError::WorkerContract(
                "Student-t baseline cannot scale sensitivity".into(),
            ));
        }
        let all_complete = runs
            .iter()
            .all(|run| run.result.fit_state == FitState::Complete);
        let mut material_shift_scenarios = Vec::new();
        let scenarios = runs
            .into_iter()
            .map(|run| {
                let shifts = [
                    (run.result.posterior.global_mean.mean - scales[0].0) / scales[0].1,
                    (run.result.posterior.between_patient_sd.mean - scales[1].0) / scales[1].1,
                    (run.result.posterior.observation_sd.mean - scales[2].0) / scales[2].1,
                    (run.result.posterior.degrees_of_freedom.mean - scales[3].0) / scales[3].1,
                ];
                let maximum = shifts
                    .iter()
                    .map(|value| value.abs())
                    .fold(0.0_f64, f64::max);
                let material = run.result.fit_state == FitState::Complete
                    && maximum >= material_standardized_shift;
                if material {
                    material_shift_scenarios.push(run.scenario.scenario_id.clone());
                }
                StudentTSensitivityScenarioResult {
                    scenario_id: run.scenario.scenario_id,
                    global_prior_sd: run.scenario.global_prior_sd,
                    between_patient_sd_prior: run.scenario.between_patient_sd_prior,
                    observation_sd_prior: run.scenario.observation_sd_prior,
                    degrees_of_freedom_excess_rate: run.scenario.degrees_of_freedom_excess_rate,
                    backend: run.result.backend,
                    request_sha256: run.result.request_sha256,
                    fit_state: run.result.fit_state,
                    sampling: run.result.sampling,
                    posterior: run.result.posterior,
                    partial_pooling: run.result.partial_pooling,
                    diagnostics: run.result.diagnostics,
                    posterior_predictive: run.result.posterior_predictive,
                    global_mean_shift_standardized: shifts[0],
                    between_patient_sd_shift_standardized: shifts[1],
                    observation_sd_shift_standardized: shifts[2],
                    degrees_of_freedom_shift_standardized: shifts[3],
                    maximum_absolute_standardized_shift: maximum,
                    material_shift: material,
                }
            })
            .collect();
        let sensitivity_state = if !all_complete {
            "diagnostic_only_nonconverged"
        } else if material_shift_scenarios.is_empty() {
            "stable_within_declared_grid"
        } else {
            "sensitive_within_declared_grid"
        };
        Ok(Self {
            format: "marklab.bayesian_student_t_hierarchy_sensitivity",
            version: 1,
            input,
            base_scenario: "baseline",
            material_standardized_shift,
            fit_state: if all_complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            sensitivity_state,
            scenarios,
            material_shift_scenarios,
            seed,
            claim_status: if all_complete {
                "experimental_prior_tail_sensitivity"
            } else {
                "diagnostic_only_prior_tail_sensitivity"
            },
        })
    }
}

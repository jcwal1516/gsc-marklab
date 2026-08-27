use std::collections::BTreeSet;

use serde::Serialize;

use crate::{
    hierarchical::{
        GaussianHierarchyInputIdentity, GaussianHierarchyPosterior,
        HierarchicalPosteriorPredictive, HierarchicalWorkerResult,
    },
    BayesError, FitState, NormalMeanDiagnostics, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct HierarchicalPriorScenario {
    pub scenario_id: String,
    pub global_prior_sd: f64,
    pub between_patient_sd_prior: f64,
}

#[derive(Debug)]
pub struct HierarchicalPriorScenarioRun {
    pub scenario: HierarchicalPriorScenario,
    pub result: HierarchicalWorkerResult,
}

#[derive(Debug, Serialize)]
pub struct HierarchicalPriorScenarioResult {
    pub scenario_id: String,
    pub global_prior_sd: f64,
    pub between_patient_sd_prior: f64,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GaussianHierarchyPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: HierarchicalPosteriorPredictive,
    pub global_mean_shift_standardized: f64,
    pub between_patient_sd_shift_standardized: f64,
    pub maximum_absolute_standardized_shift: f64,
    pub material_shift: bool,
}

#[derive(Debug, Serialize)]
pub struct HierarchicalPriorSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub input: GaussianHierarchyInputIdentity,
    pub known_sigma: f64,
    pub base_scenario: &'static str,
    pub material_standardized_shift: f64,
    pub fit_state: FitState,
    pub sensitivity_state: &'static str,
    pub scenarios: Vec<HierarchicalPriorScenarioResult>,
    pub material_shift_scenarios: Vec<String>,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl HierarchicalPriorSensitivityResult {
    pub fn new(
        input: GaussianHierarchyInputIdentity,
        known_sigma: f64,
        seed: u64,
        material_standardized_shift: f64,
        runs: Vec<HierarchicalPriorScenarioRun>,
    ) -> Result<Self, BayesError> {
        if !known_sigma.is_finite()
            || known_sigma <= 0.0
            || !material_standardized_shift.is_finite()
            || material_standardized_shift <= 0.0
            || runs.len() < 3
            || runs.len() > 9
            || runs[0].scenario.scenario_id != "baseline"
        {
            return Err(BayesError::InvalidSpec(
                "hierarchical prior-sensitivity controls are invalid".into(),
            ));
        }
        let mut names = BTreeSet::new();
        for run in &runs {
            if run.scenario.scenario_id.is_empty()
                || run.scenario.scenario_id.trim() != run.scenario.scenario_id
                || !names.insert(run.scenario.scenario_id.as_str())
                || !run.scenario.global_prior_sd.is_finite()
                || run.scenario.global_prior_sd <= 0.0
                || !run.scenario.between_patient_sd_prior.is_finite()
                || run.scenario.between_patient_sd_prior <= 0.0
            {
                return Err(BayesError::InvalidSpec(
                    "hierarchical prior scenarios are invalid".into(),
                ));
            }
        }
        let baseline_global_mean = runs[0].result.posterior.global_mean.mean;
        let baseline_global_sd = runs[0].result.posterior.global_mean.sd;
        let baseline_between_mean = runs[0].result.posterior.between_patient_sd.mean;
        let baseline_between_sd = runs[0].result.posterior.between_patient_sd.sd;
        if !finite(&[
            baseline_global_mean,
            baseline_global_sd,
            baseline_between_mean,
            baseline_between_sd,
        ]) || baseline_global_sd <= 0.0
            || baseline_between_sd <= 0.0
        {
            return Err(BayesError::WorkerContract(
                "baseline hierarchy posterior cannot scale sensitivity".into(),
            ));
        }

        let all_complete = runs
            .iter()
            .all(|run| run.result.fit_state == FitState::Complete);
        let mut material_shift_scenarios = Vec::new();
        let scenarios = runs
            .into_iter()
            .map(|run| {
                let global_shift = (run.result.posterior.global_mean.mean - baseline_global_mean)
                    / baseline_global_sd;
                let between_shift = (run.result.posterior.between_patient_sd.mean
                    - baseline_between_mean)
                    / baseline_between_sd;
                let maximum_shift = global_shift.abs().max(between_shift.abs());
                let material = run.result.fit_state == FitState::Complete
                    && maximum_shift >= material_standardized_shift;
                if material {
                    material_shift_scenarios.push(run.scenario.scenario_id.clone());
                }
                HierarchicalPriorScenarioResult {
                    scenario_id: run.scenario.scenario_id,
                    global_prior_sd: run.scenario.global_prior_sd,
                    between_patient_sd_prior: run.scenario.between_patient_sd_prior,
                    backend: run.result.backend,
                    request_sha256: run.result.request_sha256,
                    fit_state: run.result.fit_state,
                    sampling: run.result.sampling,
                    posterior: run.result.posterior,
                    diagnostics: run.result.diagnostics,
                    posterior_predictive: run.result.posterior_predictive,
                    global_mean_shift_standardized: global_shift,
                    between_patient_sd_shift_standardized: between_shift,
                    maximum_absolute_standardized_shift: maximum_shift,
                    material_shift: material,
                }
            })
            .collect::<Vec<_>>();
        let sensitivity_state = if !all_complete {
            "diagnostic_only_nonconverged"
        } else if material_shift_scenarios.is_empty() {
            "stable_within_declared_prior_grid"
        } else {
            "sensitive_within_declared_prior_grid"
        };
        Ok(Self {
            format: "marklab.bayesian_hierarchical_prior_sensitivity",
            version: 1,
            input,
            known_sigma,
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
                "experimental_prior_sensitivity"
            } else {
                "diagnostic_only_prior_sensitivity"
            },
        })
    }
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

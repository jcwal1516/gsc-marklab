use std::collections::BTreeSet;

use serde::Serialize;

use crate::{
    BayesError, DirichletMultinomialGroupInputIdentity, DirichletMultinomialGroupPosterior,
    DirichletMultinomialGroupPosteriorPredictive, DirichletMultinomialGroupWorkerResult, FitState,
    NormalMeanDiagnostics, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct DirichletMultinomialGroupSensitivityScenario {
    pub scenario_id: String,
    pub logit_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
}

#[derive(Debug)]
pub struct DirichletMultinomialGroupSensitivityScenarioRun {
    pub scenario: DirichletMultinomialGroupSensitivityScenario,
    pub result: DirichletMultinomialGroupWorkerResult,
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialGroupSensitivityScenarioResult {
    pub scenario_id: String,
    pub logit_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub concentration_prior_sd: f64,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: DirichletMultinomialGroupPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: DirichletMultinomialGroupPosteriorPredictive,
    pub class_probability_rms_standardized_shift: f64,
    pub concentration_standardized_shift: f64,
    pub maximum_absolute_standardized_shift: f64,
    pub material_shift: bool,
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialGroupSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub input: DirichletMultinomialGroupInputIdentity,
    pub base_scenario: &'static str,
    pub material_standardized_shift: f64,
    pub fit_state: FitState,
    pub sensitivity_status: &'static str,
    pub scenarios: Vec<DirichletMultinomialGroupSensitivityScenarioResult>,
    pub material_shift_scenarios: Vec<String>,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl DirichletMultinomialGroupSensitivityResult {
    pub fn new(
        input: DirichletMultinomialGroupInputIdentity,
        seed: u64,
        material_standardized_shift: f64,
        runs: Vec<DirichletMultinomialGroupSensitivityScenarioRun>,
    ) -> Result<Self, BayesError> {
        if !material_standardized_shift.is_finite()
            || material_standardized_shift <= 0.0
            || runs.len() != 7
            || runs[0].scenario.scenario_id != "baseline"
        {
            return Err(BayesError::InvalidSpec(
                "Dirichlet-multinomial sensitivity controls are invalid".into(),
            ));
        }
        let mut names = BTreeSet::new();
        for run in &runs {
            if run.scenario.scenario_id.is_empty()
                || !names.insert(run.scenario.scenario_id.as_str())
                || [
                    run.scenario.logit_prior_sd,
                    run.scenario.group_effect_prior_sd,
                    run.scenario.concentration_prior_sd,
                ]
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            {
                return Err(BayesError::InvalidSpec(
                    "Dirichlet-multinomial sensitivity scenarios are invalid".into(),
                ));
            }
        }
        let baseline = &runs[0].result;
        let baseline_classes = baseline
            .posterior
            .classes
            .iter()
            .map(|class| {
                (
                    class.class_id.clone(),
                    [
                        class.reference_probability.mean,
                        class.comparison_probability.mean,
                        class.difference_comparison_minus_reference.mean,
                    ],
                    [
                        class.reference_probability.sd,
                        class.comparison_probability.sd,
                        class.difference_comparison_minus_reference.sd,
                    ],
                )
            })
            .collect::<Vec<_>>();
        let baseline_concentration_mean = baseline.posterior.concentration.mean;
        let baseline_concentration_sd = baseline.posterior.concentration.sd;
        if baseline_classes.iter().any(|(_, _, scales)| {
            scales
                .iter()
                .any(|scale| !scale.is_finite() || *scale <= 0.0)
        }) || !baseline_concentration_sd.is_finite()
            || baseline_concentration_sd <= 0.0
        {
            return Err(BayesError::WorkerContract(
                "Dirichlet-multinomial baseline cannot scale sensitivity".into(),
            ));
        }
        let all_complete = runs
            .iter()
            .all(|run| run.result.fit_state == FitState::Complete);
        let mut material_shift_scenarios = Vec::new();
        let mut scenarios = Vec::with_capacity(runs.len());
        for run in runs {
            if run.result.posterior.classes.len() != baseline_classes.len() {
                return Err(BayesError::WorkerContract(
                    "Dirichlet-multinomial sensitivity class dimensions differ".into(),
                ));
            }
            let mut squared = 0.0;
            let mut maximum = 0.0_f64;
            for (class, (baseline_id, baseline_means, baseline_scales)) in
                run.result.posterior.classes.iter().zip(&baseline_classes)
            {
                if class.class_id != baseline_id.as_str() {
                    return Err(BayesError::WorkerContract(
                        "Dirichlet-multinomial sensitivity class identities differ".into(),
                    ));
                }
                let means = [
                    class.reference_probability.mean,
                    class.comparison_probability.mean,
                    class.difference_comparison_minus_reference.mean,
                ];
                for index in 0..3 {
                    let shift = (means[index] - baseline_means[index]) / baseline_scales[index];
                    squared += shift * shift;
                    maximum = maximum.max(shift.abs());
                }
            }
            let class_shift = (squared / (baseline_classes.len() * 3) as f64).sqrt();
            let concentration_shift = (run.result.posterior.concentration.mean
                - baseline_concentration_mean)
                / baseline_concentration_sd;
            maximum = maximum.max(concentration_shift.abs());
            let material = run.result.fit_state == FitState::Complete
                && maximum >= material_standardized_shift;
            if material {
                material_shift_scenarios.push(run.scenario.scenario_id.clone());
            }
            scenarios.push(DirichletMultinomialGroupSensitivityScenarioResult {
                scenario_id: run.scenario.scenario_id,
                logit_prior_sd: run.scenario.logit_prior_sd,
                group_effect_prior_sd: run.scenario.group_effect_prior_sd,
                concentration_prior_sd: run.scenario.concentration_prior_sd,
                backend: run.result.backend,
                request_sha256: run.result.request_sha256,
                fit_state: run.result.fit_state,
                sampling: run.result.sampling,
                posterior: run.result.posterior,
                diagnostics: run.result.diagnostics,
                posterior_predictive: run.result.posterior_predictive,
                class_probability_rms_standardized_shift: class_shift,
                concentration_standardized_shift: concentration_shift,
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
            format: "marklab.bayesian_dirichlet_multinomial_group_sensitivity",
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

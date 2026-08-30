use crate::validation::all_finite as finite;

use std::collections::BTreeSet;

use serde::Serialize;

use crate::{
    BayesError, FitState, GriddedLgcpCellPosterior, GriddedLgcpFitInputIdentity,
    GriddedLgcpFitWorkerResult, GriddedLgcpPosterior, GriddedLgcpPosteriorPredictive,
    NormalMeanDiagnostics, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct GriddedLgcpSensitivityScenario {
    pub scenario_id: String,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub covariance_sha256: String,
}

#[derive(Debug)]
pub struct GriddedLgcpSensitivityScenarioRun {
    pub scenario: GriddedLgcpSensitivityScenario,
    pub result: GriddedLgcpFitWorkerResult,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpSensitivityScenarioResult {
    pub scenario_id: String,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub covariance_sha256: String,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GriddedLgcpPosterior,
    pub cells: Vec<GriddedLgcpCellPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: GriddedLgcpPosteriorPredictive,
    pub intercept_shift_standardized: f64,
    pub coefficient_shift_standardized: f64,
    pub latent_effect_rms_shift_standardized: f64,
    pub expected_count_rms_shift_standardized: f64,
    pub maximum_absolute_standardized_shift: f64,
    pub material_shift: bool,
}

#[derive(Debug, Serialize)]
pub struct GriddedLgcpSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub input: GriddedLgcpFitInputIdentity,
    pub base_scenario: &'static str,
    pub material_standardized_shift: f64,
    pub fit_state: FitState,
    pub sensitivity_state: &'static str,
    pub scenarios: Vec<GriddedLgcpSensitivityScenarioResult>,
    pub material_shift_scenarios: Vec<String>,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl GriddedLgcpSensitivityResult {
    pub fn new(
        input: GriddedLgcpFitInputIdentity,
        seed: u64,
        material_standardized_shift: f64,
        runs: Vec<GriddedLgcpSensitivityScenarioRun>,
    ) -> Result<Self, BayesError> {
        if !material_standardized_shift.is_finite()
            || material_standardized_shift <= 0.0
            || runs.len() != 9
            || runs[0].scenario.scenario_id != "baseline"
        {
            return Err(BayesError::InvalidSpec(
                "gridded LGCP sensitivity controls are invalid".into(),
            ));
        }
        let mut names = BTreeSet::new();
        for run in &runs {
            let scenario = &run.scenario;
            if scenario.scenario_id.is_empty()
                || scenario.scenario_id.trim() != scenario.scenario_id
                || !names.insert(scenario.scenario_id.as_str())
                || !finite(&[
                    scenario.intercept_prior_sd,
                    scenario.coefficient_prior_sd,
                    scenario.field_amplitude,
                    scenario.field_length_scale_um,
                ])
                || scenario.intercept_prior_sd <= 0.0
                || scenario.coefficient_prior_sd <= 0.0
                || scenario.field_amplitude <= 0.0
                || scenario.field_length_scale_um <= 0.0
            {
                return Err(BayesError::InvalidSpec(
                    "gridded LGCP sensitivity scenarios are invalid".into(),
                ));
            }
        }
        let baseline = &runs[0].result;
        if baseline.cells.is_empty() {
            return Err(BayesError::WorkerContract(
                "gridded LGCP sensitivity baseline has no cells".into(),
            ));
        }
        if runs
            .iter()
            .any(|run| run.result.cells.len() != baseline.cells.len())
        {
            return Err(BayesError::WorkerContract(
                "gridded LGCP sensitivity cell dimensions differ".into(),
            ));
        }
        let baseline_intercept = (
            baseline.posterior.intercept.mean,
            baseline.posterior.intercept.sd,
        );
        let baseline_coefficient = (
            baseline.posterior.coefficient.mean,
            baseline.posterior.coefficient.sd,
        );
        let baseline_latent = baseline
            .cells
            .iter()
            .map(|cell| (cell.latent_effect.mean, cell.latent_effect.sd))
            .collect::<Vec<_>>();
        let baseline_expected = baseline
            .cells
            .iter()
            .map(|cell| (cell.expected_count.mean, cell.expected_count.sd))
            .collect::<Vec<_>>();
        let all_complete = runs
            .iter()
            .all(|run| run.result.fit_state == FitState::Complete);
        let mut material_shift_scenarios = Vec::new();
        let scenarios = runs
            .into_iter()
            .map(|run| {
                let intercept_shift = standardized_shift(
                    run.result.posterior.intercept.mean,
                    baseline_intercept.0,
                    baseline_intercept.1,
                );
                let coefficient_shift = standardized_shift(
                    run.result.posterior.coefficient.mean,
                    baseline_coefficient.0,
                    baseline_coefficient.1,
                );
                let latent_shift = rms_cell_shift(&run.result.cells, &baseline_latent, |cell| {
                    &cell.latent_effect
                });
                let expected_shift =
                    rms_cell_shift(&run.result.cells, &baseline_expected, |cell| {
                        &cell.expected_count
                    });
                let maximum_shift = intercept_shift
                    .abs()
                    .max(coefficient_shift.abs())
                    .max(latent_shift)
                    .max(expected_shift);
                let material = run.result.fit_state == FitState::Complete
                    && maximum_shift >= material_standardized_shift;
                if material {
                    material_shift_scenarios.push(run.scenario.scenario_id.clone());
                }
                GriddedLgcpSensitivityScenarioResult {
                    scenario_id: run.scenario.scenario_id,
                    intercept_prior_sd: run.scenario.intercept_prior_sd,
                    coefficient_prior_sd: run.scenario.coefficient_prior_sd,
                    field_amplitude: run.scenario.field_amplitude,
                    field_length_scale_um: run.scenario.field_length_scale_um,
                    covariance_sha256: run.scenario.covariance_sha256,
                    backend: run.result.backend,
                    request_sha256: run.result.request_sha256,
                    fit_state: run.result.fit_state,
                    sampling: run.result.sampling,
                    posterior: run.result.posterior,
                    cells: run.result.cells,
                    diagnostics: run.result.diagnostics,
                    posterior_predictive: run.result.posterior_predictive,
                    intercept_shift_standardized: intercept_shift,
                    coefficient_shift_standardized: coefficient_shift,
                    latent_effect_rms_shift_standardized: latent_shift,
                    expected_count_rms_shift_standardized: expected_shift,
                    maximum_absolute_standardized_shift: maximum_shift,
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
            format: "marklab.bayesian_gridded_lgcp_sensitivity",
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
                "experimental_prior_kernel_sensitivity"
            } else {
                "diagnostic_only_prior_kernel_sensitivity"
            },
        })
    }
}

fn standardized_shift(value: f64, baseline: f64, scale: f64) -> f64 {
    (value - baseline) / scale
}

fn rms_cell_shift<F>(values: &[GriddedLgcpCellPosterior], baseline: &[(f64, f64)], select: F) -> f64
where
    F: Fn(&GriddedLgcpCellPosterior) -> &crate::SarScalarSummary,
{
    if values.len() != baseline.len() {
        return f64::INFINITY;
    }
    (values
        .iter()
        .zip(baseline)
        .map(|(value, base)| {
            let value = select(value);
            ((value.mean - base.0) / base.1).powi(2)
        })
        .sum::<f64>()
        / baseline.len() as f64)
        .sqrt()
}

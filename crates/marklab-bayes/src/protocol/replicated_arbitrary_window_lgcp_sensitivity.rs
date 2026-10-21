//! Typed messages and scientific checks for replicated arbitrary window lgcp sensitivity.

use super::replicated_arbitrary_window_lgcp_fit::ResultDocument;
use crate::McseScalarSummary;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Scenario {
    pub name: String,
    pub patient_sd_prior_scale: f64,
    pub pattern_sd_prior_scale: f64,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub fit: ResultDocument,
    pub intercept_standardized_shift: f64,
    pub group_effect_standardized_shift: f64,
    pub covariate_effect_standardized_shift: f64,
    pub patient_sd_standardized_shift: f64,
    pub pattern_sd_standardized_shift: f64,
    pub patient_effect_rms_standardized_shift: f64,
    pub pattern_effect_rms_standardized_shift: f64,
    pub maximum_standardized_shift: f64,
    pub material_change: bool,
}

#[derive(Debug, Serialize)]
pub struct SensitivityResult {
    pub format: String,
    pub version: u32,
    pub scenarios: Vec<Scenario>,
    pub all_fits_complete: bool,
    pub maximum_standardized_shift: f64,
    pub material_standardized_shift: f64,
    pub material_scenarios: Vec<String>,
    pub total_draw_node_work: u64,
    pub statistical_unit: StatisticalUnit,
    pub assumptions: Vec<String>,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

/// Prior and kernel settings that distinguish one sensitivity scenario.
#[derive(Clone, Copy, Debug)]
pub struct ScenarioPriors {
    pub patient_sd_prior_scale: f64,
    pub pattern_sd_prior_scale: f64,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
}

pub fn scenario(
    name: &str,
    priors: ScenarioPriors,
    fit: ResultDocument,
    shifts: [f64; 7],
    threshold: f64,
) -> Scenario {
    let maximum_standardized_shift = shifts.into_iter().fold(0.0_f64, f64::max);
    Scenario {
        name: name.into(),
        patient_sd_prior_scale: priors.patient_sd_prior_scale,
        pattern_sd_prior_scale: priors.pattern_sd_prior_scale,
        field_amplitude: priors.field_amplitude,
        field_length_scale_um: priors.field_length_scale_um,
        fit,
        intercept_standardized_shift: shifts[0],
        group_effect_standardized_shift: shifts[1],
        covariate_effect_standardized_shift: shifts[2],
        patient_sd_standardized_shift: shifts[3],
        pattern_sd_standardized_shift: shifts[4],
        patient_effect_rms_standardized_shift: shifts[5],
        pattern_effect_rms_standardized_shift: shifts[6],
        maximum_standardized_shift,
        material_change: maximum_standardized_shift >= threshold,
    }
}

pub fn standardized_shifts(fit: &ResultDocument, baseline: &ResultDocument) -> [f64; 7] {
    [
        scalar_shift(&fit.posterior.intercept, &baseline.posterior.intercept),
        scalar_shift(
            &fit.posterior.group_effect,
            &baseline.posterior.group_effect,
        ),
        scalar_shift(
            &fit.posterior.covariate_effect,
            &baseline.posterior.covariate_effect,
        ),
        scalar_shift(&fit.posterior.patient_sd, &baseline.posterior.patient_sd),
        scalar_shift(&fit.posterior.pattern_sd, &baseline.posterior.pattern_sd),
        effect_rms_shift(
            fit.patient_effects.iter().map(|value| &value.effect),
            baseline.patient_effects.iter().map(|value| &value.effect),
        ),
        effect_rms_shift(
            fit.pattern_effects.iter().map(|value| &value.effect),
            baseline.pattern_effects.iter().map(|value| &value.effect),
        ),
    ]
}

fn scalar_shift(value: &McseScalarSummary, baseline: &McseScalarSummary) -> f64 {
    (value.mean - baseline.mean).abs() / baseline.sd.max(f64::EPSILON)
}

fn effect_rms_shift<'a>(
    values: impl Iterator<Item = &'a McseScalarSummary>,
    baselines: impl Iterator<Item = &'a McseScalarSummary>,
) -> f64 {
    let pairs = values.zip(baselines).collect::<Vec<_>>();
    let difference = (pairs
        .iter()
        .map(|(value, baseline)| (value.mean - baseline.mean).powi(2))
        .sum::<f64>()
        / pairs.len() as f64)
        .sqrt();
    let scale = (pairs
        .iter()
        .map(|(_, baseline)| baseline.sd.powi(2))
        .sum::<f64>()
        / pairs.len() as f64)
        .sqrt()
        .max(f64::EPSILON);
    difference / scale
}

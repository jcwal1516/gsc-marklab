//! Typed messages and scientific checks for conditional multitype mark sensitivity.

use super::conditional_multitype_mark::Output as FitOutput;
use crate::FitState;
use crate::McseScalarSummary;
use crate::SensitivityStatus;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
pub struct AffinityRow {
    pub type_a: String,
    pub type_b: String,
    pub contrast: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct ScoreRows {
    pub spatial_composite_log_score: McseScalarSummary,
}

#[derive(Debug, Deserialize)]
pub struct View {
    pub fit_state: FitState,
    pub pair_affinity_contrasts: Vec<AffinityRow>,
    pub comparison: ScoreRows,
}

#[derive(Debug, Serialize)]
pub struct Scenario {
    pub scenario: &'static str,
    pub intercept_prior_sd: f64,
    pub interaction_prior_sd: f64,
    pub maximum_affinity_shift_sd: f64,
    pub composite_log_score_shift_sd: f64,
    pub material: bool,
    pub result: FitOutput,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub format: &'static str,
    pub version: u32,
    pub baseline_scenario: &'static str,
    pub materiality_threshold_sd: f64,
    pub maximum_total_draw_parameter_work: u64,
    pub scenarios: Vec<Scenario>,
    pub fit_state: FitState,
    pub sensitivity_status: SensitivityStatus,
    pub statistical_unit: StatisticalUnit,
    pub assumptions: [&'static str; 4],
    pub finite_result_policy: &'static str,
    pub claim_status: ClaimStatus,
}

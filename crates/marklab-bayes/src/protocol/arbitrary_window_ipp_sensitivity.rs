//! Typed messages and scientific checks for arbitrary window ipp sensitivity.

use super::arbitrary_window_ipp_fit::ArbitraryWindowIppFitInputIdentity;
use super::arbitrary_window_ipp_fit::ArbitraryWindowIppFitPosterior;
use crate::FitState;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::ScalarSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct SensitivityScenario {
    pub name: String,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_sd: f64,
    pub backend: WorkerBackend,
    pub input: ArbitraryWindowIppFitInputIdentity,
    pub request_sha256: String,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: ArbitraryWindowIppFitPosterior,
    pub diagnostics: PosteriorDiagnostics,
    pub total_expected_count_mean: f64,
    pub intercept_standardized_shift: f64,
    pub coefficient_standardized_shift: f64,
    pub maximum_standardized_shift: f64,
    pub material_change: bool,
}

#[derive(Debug, Serialize)]
pub struct SensitivityResult {
    pub format: String,
    pub version: u32,
    pub scenarios: Vec<SensitivityScenario>,
    pub all_fits_complete: bool,
    pub material_standardized_shift: f64,
    pub material_scenarios: Vec<String>,
    pub fit_count: usize,
    pub total_draw_node_work: u64,
    pub statistical_unit: StatisticalUnit,
    pub claim_status: ClaimStatus,
}

pub fn standardized(summary: &ScalarSummary, baseline: f64, baseline_sd: f64) -> f64 {
    (summary.mean - baseline).abs() / baseline_sd.max(f64::EPSILON)
}

//! Typed messages and scientific checks for arbitrary window ipp quadrature sensitivity.

use super::arbitrary_window_ipp_fit::ArbitraryWindowIppFitInputIdentity;
use super::arbitrary_window_ipp_fit::ArbitraryWindowIppFitPosterior;
use crate::FitState;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct ResolutionResult {
    pub name: String,
    pub quadrature_node_count: usize,
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
    pub total_count_relative_shift: f64,
    pub maximum_standardized_shift: f64,
}

#[derive(Debug, Serialize)]
pub struct QuadratureSensitivityResult {
    pub format: String,
    pub version: u32,
    pub resolutions: Vec<ResolutionResult>,
    pub all_fits_complete: bool,
    pub maximum_standardized_shift: f64,
    pub material_standardized_shift: f64,
    pub material_change: bool,
    pub total_draw_node_work: u64,
    pub statistical_unit: StatisticalUnit,
    pub claim_status: ClaimStatus,
}

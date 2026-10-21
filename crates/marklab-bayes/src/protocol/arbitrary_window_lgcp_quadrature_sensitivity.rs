//! Typed messages and scientific checks for arbitrary window lgcp quadrature sensitivity.

use super::arbitrary_window_lgcp_fit::InputIdentity;
use super::arbitrary_window_lgcp_fit::SpatialPpc;
use crate::FitState;
use crate::GriddedLgcpPosterior;
use crate::PosteriorDiagnostics;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct Resolution {
    pub name: String,
    pub quadrature_node_count: usize,
    pub backend: WorkerBackend,
    pub source_backend: WorkerBackend,
    pub input: InputIdentity,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub posterior: GriddedLgcpPosterior,
    pub diagnostics: PosteriorDiagnostics,
    pub spatial_posterior_predictive: Option<SpatialPpc>,
    pub total_expected_count_mean: f64,
    pub intercept_standardized_shift: f64,
    pub coefficient_standardized_shift: f64,
    pub total_count_relative_shift: f64,
    pub maximum_standardized_shift: f64,
}

#[derive(Debug, Serialize)]
pub struct SensitivityResult {
    pub format: String,
    pub version: u32,
    pub resolutions: Vec<Resolution>,
    pub all_fits_complete: bool,
    pub maximum_standardized_shift: f64,
    pub material_standardized_shift: f64,
    pub material_change: bool,
    pub total_draw_node_work: u64,
    pub statistical_unit: StatisticalUnit,
    pub claim_status: ClaimStatus,
}

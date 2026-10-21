//! Typed messages and scientific checks for arbitrary window lgcp sensitivity.

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
pub struct Scenario {
    pub name: String,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub backend: WorkerBackend,
    pub source_backend: WorkerBackend,
    pub input: InputIdentity,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub posterior: GriddedLgcpPosterior,
    pub diagnostics: PosteriorDiagnostics,
    pub spatial_posterior_predictive: Option<SpatialPpc>,
    pub intercept_standardized_shift: f64,
    pub coefficient_standardized_shift: f64,
    pub latent_field_rms_standardized_shift: f64,
    pub expected_count_rms_relative_shift: f64,
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
    pub claim_status: ClaimStatus,
}

pub fn rms_difference(left: impl Iterator<Item = f64>, right: impl Iterator<Item = f64>) -> f64 {
    let differences = left
        .zip(right)
        .map(|(left, right)| (left - right).powi(2))
        .collect::<Vec<_>>();
    (differences.iter().sum::<f64>() / differences.len() as f64).sqrt()
}

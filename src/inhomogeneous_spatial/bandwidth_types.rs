use serde::{Deserialize, Serialize};

use super::{
    InhomogeneousSpatialConfig, InhomogeneousSpatialError, InhomogeneousSpatialLimits,
    InhomogeneousSpatialResult,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Work ceilings specific to prespecified Gaussian bandwidth selection.
pub struct GaussianBandwidthSelectionLimits {
    /// Maximum number of ordered candidate bandwidths.
    pub maximum_candidates: usize,
    /// Maximum kernel/probe evaluations across all candidate event fits.
    pub maximum_intensity_evaluations: usize,
}

impl GaussianBandwidthSelectionLimits {
    /// Construct positive selection ceilings.
    pub fn new(
        maximum_candidates: usize,
        maximum_intensity_evaluations: usize,
    ) -> Result<Self, InhomogeneousSpatialError> {
        if maximum_candidates == 0 || maximum_intensity_evaluations == 0 {
            return Err(InhomogeneousSpatialError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_candidates,
            maximum_intensity_evaluations,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Prespecified likelihood selection followed by the existing Gaussian K/L analysis.
pub struct GaussianBandwidthSelectionConfig {
    pub(super) radii_um: Box<[f64]>,
    pub(super) candidate_bandwidths_um: Box<[f64]>,
    pub(super) integration_grid: [usize; 2],
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) minimum_intensity_per_um2: f64,
    pub(super) analysis_limits: InhomogeneousSpatialLimits,
    pub(super) selection_limits: GaussianBandwidthSelectionLimits,
}

impl GaussianBandwidthSelectionConfig {
    /// Validate ordered candidates and all downstream K/L controls.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        radii_um: Vec<f64>,
        candidate_bandwidths_um: Vec<f64>,
        integration_grid: [usize; 2],
        simulations: usize,
        seed: u64,
        alpha: f64,
        minimum_intensity_per_um2: f64,
        analysis_limits: InhomogeneousSpatialLimits,
        selection_limits: GaussianBandwidthSelectionLimits,
    ) -> Result<Self, InhomogeneousSpatialError> {
        if candidate_bandwidths_um.is_empty()
            || candidate_bandwidths_um.len() > selection_limits.maximum_candidates
            || candidate_bandwidths_um
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            || candidate_bandwidths_um
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(InhomogeneousSpatialError::InvalidConfig(
                "candidate bandwidths must be nonempty, bounded, finite, positive, and increasing"
                    .into(),
            ));
        }
        InhomogeneousSpatialConfig::new(
            radii_um.clone(),
            candidate_bandwidths_um[0],
            integration_grid,
            simulations,
            seed,
            alpha,
            minimum_intensity_per_um2,
            analysis_limits,
        )?;
        Ok(Self {
            radii_um: radii_um.into_boxed_slice(),
            candidate_bandwidths_um: candidate_bandwidths_um.into_boxed_slice(),
            integration_grid,
            simulations,
            seed,
            alpha,
            minimum_intensity_per_um2,
            analysis_limits,
            selection_limits,
        })
    }

    /// Ordered prespecified candidate bandwidths in micrometres.
    pub fn candidate_bandwidths_um(&self) -> &[f64] {
        &self.candidate_bandwidths_um
    }

    /// Downstream K/L resource controls.
    pub fn analysis_limits(&self) -> InhomogeneousSpatialLimits {
        self.analysis_limits
    }

    /// Selection-specific work controls.
    pub fn selection_limits(&self) -> GaussianBandwidthSelectionLimits {
        self.selection_limits
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Persisted leave-one-out likelihood score for one candidate bandwidth.
pub struct GaussianBandwidthCandidateScore {
    /// Candidate bandwidth in micrometres.
    pub bandwidth_um: f64,
    /// Mean log leave-one-out event intensity.
    pub mean_log_leave_one_out_intensity: f64,
    /// Smallest leave-one-out event intensity.
    pub minimum_intensity_per_um2: f64,
    /// Largest leave-one-out event intensity.
    pub maximum_intensity_per_um2: f64,
    /// Intensity evaluations charged for this candidate.
    pub intensity_evaluations: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Complete identity and score table for prespecified bandwidth selection.
pub struct GaussianBandwidthSelectionSummary {
    /// Stable selection method.
    pub method: String,
    /// Stable score objective.
    pub objective: String,
    /// Exact-tie policy.
    pub tie_break: String,
    /// Selection cannot inspect downstream K/L/g values.
    pub selection_uses_spatial_curve: bool,
    /// Zero-based selected candidate index.
    pub selected_index: usize,
    /// Selected bandwidth in micrometres.
    pub selected_bandwidth_um: f64,
    /// Ordered complete candidate score table.
    pub candidates: Vec<GaussianBandwidthCandidateScore>,
    /// Total selection-only intensity evaluations.
    pub intensity_evaluations: usize,
    /// Exact selection artifact identity.
    pub artifact_digest: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Selected-bandwidth wrapper around the existing typed Gaussian K/L result.
pub struct SelectedInhomogeneousSpatialResult {
    /// Persisted prespecified selection evidence.
    pub selection: GaussianBandwidthSelectionSummary,
    /// Existing K/L result evaluated only at the selected bandwidth.
    pub analysis: InhomogeneousSpatialResult,
    /// Exact selection-plus-analysis configuration identity.
    pub configuration_digest: String,
    /// Conservative retained output storage.
    pub estimated_storage_bytes: usize,
    /// Selection-specific work controls.
    pub selection_limits: GaussianBandwidthSelectionLimits,
}

use serde::{Deserialize, Serialize};

use crate::config::ComponentMode;

use super::{
    common::{
        default_true, AnalysisSection, AnalysisStatus, Interpretation, StatusFlag, TimingStage,
    },
    diagnostics::DiagnosticsResult,
    prepost::CurveComparisonResult,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MarkedPatternResult {
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
    pub mark_label: String,
    pub status: AnalysisStatus,
    pub status_flags: Vec<StatusFlag>,
    pub n_cells: usize,
    pub n_marked: usize,
    pub p_hat: f64,
    pub window: WindowSummary,
    #[serde(default)]
    pub qc: QcSummary,
    pub primary_endpoint: PrimaryEndpoint,
    pub spectrum: AnalysisSection<SpectrumSummary>,
    #[serde(default)]
    pub spectrum_null_sensitivity: AnalysisSection<SpectrumNullSensitivitySummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spectrum_curve: Vec<SpectrumPoint>,
    #[serde(default)]
    pub mark_pair_covariance: AnalysisSection<FunctionalSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mark_pair_covariance_curve: Vec<MarkPairCovariancePoint>,
    pub anisotropy: AnalysisSection<AnisotropySummary>,
    pub multiscale_residual: AnalysisSection<MultiscaleResidualSummary>,
    #[serde(default)]
    pub scale_energy: AnalysisSection<FunctionalSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scale_energy_curve: Vec<ScaleEnergyPoint>,
    #[serde(default)]
    pub residual_territories: AnalysisSection<Vec<ResidualTerritory>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prepost_curve_comparisons: Vec<CurveComparisonResult>,
    pub component_mode_selection: ComponentModeSelection,
    pub component_results: AnalysisSection<Vec<ComponentAnalysisSummary>>,
    #[serde(default)]
    pub diagnostics: AnalysisSection<DiagnosticsResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timings: Vec<TimingStage>,
    pub interpretation: Interpretation,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WindowSummary {
    pub area_um2: f64,
    pub analysis_effective_length_um: f64,
    pub d_nn_mean_um: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QcSummary {
    pub valid_mask_fraction: f64,
    #[serde(default)]
    pub valid_tumor_fraction: Option<f64>,
    #[serde(default)]
    pub valid_ihc_fraction: Option<f64>,
    pub internal_control_valid_fraction: Option<f64>,
    pub artifact_excluded_fraction: Option<f64>,
    pub nonviable_excluded_fraction: Option<f64>,
    pub mean_tumor_probability: Option<f64>,
    pub mean_nucleus_area_um2: Option<f64>,
    pub tumor_cell_density_per_mm2: Option<f64>,
}

impl Default for QcSummary {
    fn default() -> Self {
        Self {
            valid_mask_fraction: 1.0,
            valid_tumor_fraction: None,
            valid_ihc_fraction: None,
            internal_control_valid_fraction: None,
            artifact_excluded_fraction: None,
            nonviable_excluded_fraction: None,
            mean_tumor_probability: None,
            mean_nucleus_area_um2: None,
            tumor_cell_density_per_mm2: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrimaryEndpoint {
    pub name: PrimaryEndpointKind,
    pub value: AnalysisSection<f64>,
    pub p_value: AnalysisSection<f64>,
    pub null: SpectrumNullModel,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrimaryEndpointKind {
    LowKExcess,
    ComponentLowKExcess,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpectrumSummary {
    pub max_interpretable_scale_um: f64,
    pub k_min: Option<f64>,
    pub k_max: Option<f64>,
    pub n_k_modes: usize,
    pub n_shells: usize,
    #[serde(default)]
    pub n_permutations: usize,
    pub spectral_curve_test: AnalysisSection<FunctionalSummary>,
    pub xi_um: Option<f64>,
    pub xi_stability_interval_um: Option<[f64; 2]>,
    pub low_k_excess: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_k_excess_p_value: Option<f64>,
    pub alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xi_um_p_value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_p_value: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpectrumNullModel {
    FixedPositionRandomLabeling,
    StratifiedFixedPositionRandomLabeling,
    ComponentSpecificFixedPositionRandomLabeling,
}

impl SpectrumNullModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FixedPositionRandomLabeling => "fixed_position_random_labeling",
            Self::StratifiedFixedPositionRandomLabeling => {
                "stratified_fixed_position_random_labeling"
            }
            Self::ComponentSpecificFixedPositionRandomLabeling => {
                "component_specific_fixed_position_random_labeling"
            }
        }
    }
}

impl std::fmt::Display for SpectrumNullModel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpectrumConfoundingConclusion {
    ConfoundedBySpatialStrata,
    BothSignificant,
    NoUnstratifiedSignal,
    DegenerateStratifiedNull,
    NotEvaluable,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpectrumNullInferenceSummary {
    pub p_global: f64,
    pub low_k_excess_p_value: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpectrumNullSensitivitySummary {
    pub primary_null: SpectrumNullModel,
    pub family_wise_alpha: f64,
    pub unstratified: AnalysisSection<SpectrumNullInferenceSummary>,
    pub stratified: AnalysisSection<SpectrumNullInferenceSummary>,
    pub conclusion: SpectrumConfoundingConclusion,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpectrumPoint {
    pub k: f64,
    pub observed_power: f64,
    pub median_permutation_power: f64,
    pub whitened_power: f64,
    #[serde(default = "default_true")]
    pub inference_eligible: bool,
    pub lower_global_envelope: Option<f64>,
    pub upper_global_envelope: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FunctionalSummary {
    pub p_global: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub erl_depth: Option<f64>,
    #[serde(default)]
    pub n_permutations: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MarkPairCovariancePoint {
    pub r_min_um: f64,
    pub r_max_um: f64,
    pub covariance: Option<f64>,
    #[serde(default = "default_true")]
    pub inference_eligible: bool,
    pub lower_global_envelope: Option<f64>,
    pub upper_global_envelope: Option<f64>,
    pub pair_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResidualTerritory {
    pub center_x_um: f64,
    pub center_y_um: f64,
    pub radius_um: f64,
    pub analysis_scale_um: f64,
    pub residual_score: f64,
    pub supporting_marked_cells: usize,
    pub component_id: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ComponentAnalysisSummary {
    pub component_id: u32,
    pub n_cells: usize,
    pub n_marked: usize,
    pub p_hat: f64,
    pub status_flags: Vec<StatusFlag>,
    pub primary_endpoint_value: AnalysisSection<f64>,
    pub p_global: Option<f64>,
    pub k_min: Option<f64>,
    pub k_max: Option<f64>,
    pub n_k_modes: usize,
    pub xi_um: Option<f64>,
    pub alpha: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ComponentModeSelection {
    pub requested: ComponentMode,
    pub selected: ResolvedComponentMode,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedComponentMode {
    Pooled,
    Separate,
    Both,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScaleEnergyPoint {
    pub band: ScaleEnergyBand,
    pub scale_um: f64,
    pub energy_fraction: f64,
    #[serde(default = "default_true")]
    pub inference_eligible: bool,
    pub lower_global_envelope: Option<f64>,
    pub upper_global_envelope: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScaleEnergyBand {
    LocalDifference,
    Residual,
    BlockMean,
}

impl ScaleEnergyBand {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalDifference => "local_difference",
            Self::Residual => "residual",
            Self::BlockMean => "block_mean",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AnisotropySummary {
    pub index: f64,
    pub theta_deg: Option<f64>,
    pub p_value: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MultiscaleResidualSummary {
    pub local_difference_energy_fraction: f64,
    pub residual_energy_fraction: f64,
    pub block_mean_variance_fraction: f64,
    pub block_mean_to_local_difference_ratio: Option<f64>,
    pub territory_count: usize,
    pub block_mean_variance_fraction_p_value: AnalysisSection<f64>,
    pub territory_count_p_value: AnalysisSection<f64>,
}

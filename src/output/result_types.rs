use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

use crate::multimodal::cell_table::FusedCell;

pub const RESULT_FORMAT_VERSION: &str = "0.3";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResultDocument {
    pub format_version: String,
    pub provenance: Provenance,
    pub analysis: AnalysisResult,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub program: String,
    pub crate_version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", content = "result", rename_all = "snake_case")]
// The unboxed variants are part of the result-format 0.3 Rust contract.
#[allow(clippy::large_enum_variant)]
pub enum AnalysisResult {
    MarkedPattern(MarkedPatternResult),
    Multimodal(MultimodalResult),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MultimodalResult {
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
    pub status: String,
    pub registration: AnalysisSection<RegistrationSummary>,
    pub fused_cell_summary: AnalysisSection<FusedCellSummary>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub fused_cells: Vec<FusedCell>,
    pub neighborhood_enrichment: AnalysisSection<Vec<NeighborhoodEnrichmentResult>>,
    pub cross_interaction_curves: AnalysisSection<Vec<CrossInteractionCurve>>,
    pub neighborhood_territories: AnalysisSection<Vec<TerritoryFeature>>,
    pub territory_profiles: AnalysisSection<Vec<TerritoryProfile>>,
    pub territory_comparisons: AnalysisSection<Vec<CurveTestResult>>,
    pub diagnostics: AnalysisSection<DiagnosticsResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timings: Vec<TimingStage>,
    pub interpretation: Interpretation,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AnalysisSection<T> {
    Available {
        value: T,
    },
    Disabled,
    #[default]
    NotApplicable,
    InsufficientData {
        reason: String,
    },
}

impl<T> AnalysisSection<T> {
    pub fn available(value: T) -> Self {
        Self::Available { value }
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Available { value } => Some(value),
            Self::Disabled | Self::NotApplicable | Self::InsufficientData { .. } => None,
        }
    }

    pub fn value_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Available { value } => Some(value),
            Self::Disabled | Self::NotApplicable | Self::InsufficientData { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ArtifactStatus {
    Written { path: PathBuf },
    Disabled,
    NotApplicable,
    InsufficientData { reason: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OutputManifest {
    pub result: ArtifactStatus,
    pub artifacts: BTreeMap<String, ArtifactStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MarkedPatternResult {
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
    pub mark_label: String,
    pub status: String,
    pub status_flags: Vec<StatusFlag>,
    pub n_cells: usize,
    pub n_marked: usize,
    pub p_hat: f64,
    pub window: WindowSummary,
    #[serde(default)]
    pub qc: QcSummary,
    pub primary_endpoint: PrimaryEndpoint,
    pub spectrum: AnalysisSection<SpectrumSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spectrum_curve: Vec<SpectrumPoint>,
    #[serde(default)]
    pub pair_correlation: AnalysisSection<FunctionalSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pair_correlation_curve: Vec<PairCorrelationPoint>,
    pub anisotropy: AnalysisSection<AnisotropySummary>,
    pub wavelet: AnalysisSection<WaveletSummary>,
    #[serde(default)]
    pub scalogram: AnalysisSection<FunctionalSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scalogram_curve: Vec<ScalogramPoint>,
    #[serde(default)]
    pub wavelet_territories: AnalysisSection<Vec<TerritoryFeature>>,
    #[serde(default)]
    pub registration: AnalysisSection<RegistrationSummary>,
    #[serde(default)]
    pub fused_cell_summary: AnalysisSection<FusedCellSummary>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub fused_cells: Vec<FusedCell>,
    #[serde(default)]
    pub neighborhood_enrichment: AnalysisSection<Vec<NeighborhoodEnrichmentResult>>,
    #[serde(default)]
    pub cross_interaction_curves: AnalysisSection<Vec<CrossInteractionCurve>>,
    #[serde(default)]
    pub territory_profiles: AnalysisSection<Vec<TerritoryProfile>>,
    #[serde(default)]
    pub territory_comparisons: AnalysisSection<Vec<CurveTestResult>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prepost_curve_tests: Vec<CurveTestResult>,
    #[serde(default)]
    pub component_results: AnalysisSection<Vec<ComponentAnalysisSummary>>,
    #[serde(default)]
    pub diagnostics: AnalysisSection<DiagnosticsResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timings: Vec<TimingStage>,
    pub interpretation: Interpretation,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PrePostResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_flags: Vec<StatusFlag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curve_tests: Vec<CurveTestResult>,
    pub delta_xi_um: AnalysisSection<f64>,
    pub delta_low_k_excess: AnalysisSection<f64>,
    pub delta_alpha: AnalysisSection<f64>,
    pub delta_anisotropy_index: AnalysisSection<f64>,
    pub delta_coarse_variance_fraction: AnalysisSection<f64>,
    pub delta_territory_count: AnalysisSection<isize>,
    pub territory_summary: AnalysisSection<TerritoryPrePostSummary>,
    pub interpretation_text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct TerritoryPrePostSummary {
    pub pre_count: usize,
    pub post_count: usize,
    pub delta_count: isize,
    pub delta_mean_radius_um: AnalysisSection<f64>,
    pub delta_median_radius_um: AnalysisSection<f64>,
    pub delta_mean_supporting_cells: AnalysisSection<f64>,
    pub delta_median_supporting_cells: AnalysisSection<f64>,
    pub new_domain_count: usize,
    pub lost_domain_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TimingStage {
    pub stage_name: String,
    pub wall_ms: f64,
    pub cpu_threads: usize,
    pub n_cells: usize,
    pub n_marked: usize,
    pub n_k_modes: usize,
    pub n_permutations: usize,
    pub estimated_peak_memory_mib: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WindowSummary {
    pub area_um2: f64,
    pub l_eff_um: f64,
    pub d_nn_mean_um: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct QcSummary {
    pub valid_mask_fraction: f64,
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
pub struct PrimaryEndpoint {
    pub name: String,
    pub value: AnalysisSection<f64>,
    pub p_value: AnalysisSection<f64>,
    pub null: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
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
pub struct FunctionalSummary {
    pub p_global: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub erl_depth: Option<f64>,
    #[serde(default)]
    pub n_permutations: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PairCorrelationPoint {
    pub r_min_um: f64,
    pub r_max_um: f64,
    pub value: f64,
    #[serde(default = "default_true")]
    pub inference_eligible: bool,
    pub lower_global_envelope: Option<f64>,
    pub upper_global_envelope: Option<f64>,
    pub count: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct RegistrationSummary {
    pub transform_type: String,
    pub landmark_count: usize,
    pub rmse_um: f64,
    pub median_residual_um: f64,
    pub p95_residual_um: f64,
    pub max_residual_um: f64,
    pub usable_min_distance_um: f64,
    pub status: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct FusedCellSummary {
    pub n_he_cells: usize,
    pub n_ihc_cells: usize,
    pub n_fused_cells: usize,
    pub registration_error_um: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct NeighborhoodEnrichmentResult {
    pub label_a: String,
    pub label_b: String,
    pub observed_edges: usize,
    pub expected_edges: f64,
    pub enrichment_ratio: Option<f64>,
    #[serde(default)]
    pub enrichment_ratio_unavailable_reason: Option<EnrichmentStatisticUnavailableReason>,
    pub z_score: Option<f64>,
    #[serde(default)]
    pub z_score_unavailable_reason: Option<EnrichmentStatisticUnavailableReason>,
    pub p_value: Option<f64>,
    pub q_value: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentStatisticUnavailableReason {
    ZeroExpectedEdges,
    ZeroNullVariance,
    InsufficientNullSamples,
    NonFiniteComputation,
}

impl EnrichmentStatisticUnavailableReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ZeroExpectedEdges => "zero_expected_edges",
            Self::ZeroNullVariance => "zero_null_variance",
            Self::InsufficientNullSamples => "insufficient_null_samples",
            Self::NonFiniteComputation => "non_finite_computation",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct CrossInteractionCurve {
    pub label_a: String,
    pub label_b: String,
    pub points: Vec<PairCorrelationPoint>,
    pub p_global: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct TerritoryProfile {
    pub territory_id: usize,
    pub cell_type_fractions: Vec<LabelFraction>,
    pub enrichment: Vec<NeighborhoodEnrichmentResult>,
    pub cross_curves: Vec<CrossInteractionCurve>,
    pub below_registration_resolution: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct LabelFraction {
    pub label: String,
    pub fraction: f64,
    pub count: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct CurveTestResult {
    pub comparison_name: String,
    pub metric: String,
    pub statistic: f64,
    pub p_difference: Option<f64>,
    pub equivalence_margin: Option<f64>,
    pub p_equivalence: Option<f64>,
    pub equivalent: Option<bool>,
    pub interpretation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TerritoryFeature {
    pub center_x_um: f64,
    pub center_y_um: f64,
    pub radius_um: f64,
    pub scale_um: f64,
    pub z_or_power: f64,
    pub supporting_cells: usize,
    pub component_id: Option<u32>,
    pub qc_overlap_fraction: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
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

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct DiagnosticsResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beta_binomial: Option<BetaBinomialSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_smoothing: Option<GraphSmoothingSummary>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BetaBinomialSummary {
    pub diagnostic_name: String,
    pub n_cells: usize,
    pub n_marked: usize,
    pub prior_alpha: f64,
    pub prior_beta: f64,
    pub posterior_mean: f64,
    pub credible_interval_95: [f64; 2],
    pub group_posterior_mean_range: f64,
    pub groups: Vec<BetaBinomialGroupSummary>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BetaBinomialGroupSummary {
    pub group: String,
    pub n_cells: usize,
    pub n_marked: usize,
    pub posterior_mean: f64,
    pub credible_interval_95: [f64; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct GraphSmoothingSummary {
    pub diagnostic_name: String,
    pub n_nodes: usize,
    pub n_edges: usize,
    pub mean_degree: f64,
    pub below_registration_resolution_edge_fraction: f64,
    pub label_count: usize,
    pub label_pair_scores: Vec<GraphSmoothingLabelPairSummary>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct GraphSmoothingLabelPairSummary {
    pub label_a: String,
    pub label_b: String,
    pub observed_edges: usize,
    pub message_passing_score: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ScalogramPoint {
    pub band: String,
    pub scale_um: f64,
    pub energy_fraction: f64,
    #[serde(default = "default_true")]
    pub inference_eligible: bool,
    pub lower_global_envelope: Option<f64>,
    pub upper_global_envelope: Option<f64>,
}

const fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AnisotropySummary {
    pub index: f64,
    pub theta_deg: Option<f64>,
    pub p_value: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WaveletSummary {
    pub fine_variance_fraction: f64,
    pub intermediate_variance_fraction: f64,
    pub coarse_variance_fraction: f64,
    pub coarse_to_fine_ratio: Option<f64>,
    pub territory_count: usize,
    pub coarse_variance_fraction_p_value: AnalysisSection<f64>,
    pub territory_count_p_value: AnalysisSection<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Interpretation {
    pub class: String,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum StatusFlag {
    UnderpoweredTooFewCells,
    UnderpoweredTooFewMarked,
    UnderpoweredTooFewUnmarked,
    UnderpoweredAreaTooSmall,
    UnderpoweredTooFewKShells,
    InvalidIhcMask,
    InternalControlFailureOverlap,
    StainGradientSuspect,
    MaskFragmentationSuspect,
    WindowOrGriddingArtifactSuspect,
    SensitivityUnstable,
    ConfoundedBySpatialStrata,
    DegenerateSpatialStrataNull,
    PrePostNotAnatomicallyComparable,
    SuppressedBiologicInterpretation,
}

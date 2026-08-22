use serde::{Deserialize, Serialize};

use crate::multimodal::cells::FusedCell;
use crate::registration::transform::TransformKind;

use super::{
    common::{default_true, AnalysisSection, AnalysisStatus, Interpretation, TimingStage},
    diagnostics::DiagnosticsResult,
    prepost::CurveComparisonResult,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MultimodalResult {
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
    pub status: AnalysisStatus,
    pub registration: AnalysisSection<RegistrationSummary>,
    pub fused_cell_summary: AnalysisSection<FusedCellSummary>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub fused_cells: Vec<FusedCell>,
    pub neighborhood_enrichment: AnalysisSection<Vec<NeighborhoodEnrichmentResult>>,
    pub cross_interaction_curves: AnalysisSection<Vec<CrossInteractionCurve>>,
    pub neighborhood_territories: AnalysisSection<Vec<NeighborhoodTerritory>>,
    pub territory_profiles: AnalysisSection<Vec<TerritoryProfile>>,
    pub territory_comparisons: AnalysisSection<Vec<CurveComparisonResult>>,
    pub diagnostics: AnalysisSection<DiagnosticsResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timings: Vec<TimingStage>,
    pub interpretation: Interpretation,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CrossInteractionPoint {
    pub r_min_um: f64,
    pub r_max_um: f64,
    pub value: Option<f64>,
    #[serde(default = "default_true")]
    pub inference_eligible: bool,
    pub lower_global_envelope: Option<f64>,
    pub upper_global_envelope: Option<f64>,
    pub count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RegistrationSummary {
    pub transform_type: TransformKind,
    pub landmark_count: usize,
    pub rmse_um: f64,
    pub median_residual_um: f64,
    pub p95_residual_um: f64,
    pub max_residual_um: f64,
    pub usable_min_distance_um: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FusedCellSummary {
    pub n_he_cells: usize,
    pub n_ihc_cells: usize,
    pub n_fused_cells: usize,
    pub registration_error_um: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct CrossInteractionCurve {
    pub label_a: String,
    pub label_b: String,
    pub points: Vec<CrossInteractionPoint>,
    pub p_global: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TerritoryProfile {
    pub territory_id: usize,
    pub cell_type_fractions: Vec<LabelFraction>,
    pub below_registration_resolution: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LabelFraction {
    pub label: String,
    pub fraction: f64,
    pub count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NeighborhoodTerritory {
    pub center_x_um: f64,
    pub center_y_um: f64,
    pub radius_um: f64,
    pub supporting_abnormal_cells: usize,
    pub cluster_id: u32,
}

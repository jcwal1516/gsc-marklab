use serde::{Deserialize, Serialize};

use super::common::{AnalysisSection, StatusFlag};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrePostResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_flags: Vec<StatusFlag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curve_comparisons: Vec<CurveComparisonResult>,
    pub delta_xi_um: AnalysisSection<f64>,
    pub delta_low_k_excess: AnalysisSection<f64>,
    pub delta_alpha: AnalysisSection<f64>,
    pub delta_anisotropy_index: AnalysisSection<f64>,
    pub delta_block_mean_variance_fraction: AnalysisSection<f64>,
    pub delta_territory_count: AnalysisSection<isize>,
    pub territory_summary: AnalysisSection<TerritoryPrePostSummary>,
    pub interpretation_text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct CurveComparisonResult {
    pub comparison_name: String,
    pub method: CurveComparisonMethod,
    pub metric: String,
    pub availability: CurveComparisonAvailability,
    pub statistic: Option<f64>,
    #[serde(default)]
    pub unavailable_reason: Option<String>,
    pub pooled_bin_p_value: Option<f64>,
    pub margin: Option<f64>,
    pub within_margin: Option<bool>,
    pub interpretation: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CurveComparisonMethod {
    PooledBinPermutation,
    DescriptiveMargin,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CurveComparisonAvailability {
    Available,
    InsufficientData,
}

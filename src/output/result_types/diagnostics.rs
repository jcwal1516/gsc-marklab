use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticsResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beta_posterior_groups: Option<BetaPosteriorSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_smoothing: Option<GraphSmoothingSummary>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BetaPosteriorSummary {
    pub diagnostic_name: String,
    pub n_cells: usize,
    pub n_marked: usize,
    pub prior_alpha: f64,
    pub prior_beta: f64,
    pub posterior_mean: f64,
    pub credible_interval_95: [f64; 2],
    pub group_posterior_mean_range: f64,
    pub groups: Vec<BetaPosteriorGroupSummary>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BetaPosteriorGroupSummary {
    pub group: String,
    pub n_cells: usize,
    pub n_marked: usize,
    pub posterior_mean: f64,
    pub credible_interval_95: [f64; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct GraphSmoothingLabelPairSummary {
    pub label_a: String,
    pub label_b: String,
    pub observed_edges: usize,
    pub message_passing_score: f64,
}

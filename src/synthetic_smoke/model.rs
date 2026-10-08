use std::collections::BTreeMap;

use serde::Serialize;

use crate::output::StatusFlag;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SyntheticSmokeSummary {
    pub suite: String,
    pub suite_kind: &'static str,
    pub seed: u64,
    pub engine_version: &'static str,
    pub configuration: MarkedSmokeConfiguration,
    pub replicates: usize,
    pub status: String,
    pub alpha: f64,
    pub generators: Vec<&'static str>,
    pub results: BTreeMap<String, SyntheticSmokeResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MarkedSmokeConfiguration {
    pub permutations: usize,
    pub permutation_seed: u64,
    pub threads: usize,
    pub family_wise_alpha: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SyntheticSmokeResult {
    pub replicates_attempted: usize,
    pub replicates_completed: usize,
    pub replicates_failed: usize,
    pub failure_reasons: Vec<String>,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_low_k_excess: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_i_error_alpha_0_05: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_i_error_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_anisotropy_index: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_territory_count: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppression_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppression_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepost_incomparable_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepost_incomparable_confidence_interval: Option<BinomialConfidenceInterval>,
    pub acceptance_criterion: &'static str,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_flags: Vec<StatusFlag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalSyntheticSmokeSummary {
    pub suite: String,
    pub suite_kind: &'static str,
    pub seed: u64,
    pub engine_version: &'static str,
    pub configuration: MultimodalSmokeConfiguration,
    #[serde(flatten)]
    pub results: BTreeMap<String, MultimodalSyntheticSmokeResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalSmokeConfiguration {
    pub permutations: usize,
    pub permutation_seed_base: u64,
    pub permutation_seed_policy: &'static str,
    pub radius_um: f64,
    pub null_models: Vec<String>,
    pub cross_interaction_margin: f64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct BinomialConfidenceInterval {
    pub confidence_level: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalSyntheticSmokeResult {
    pub replicates_attempted: usize,
    pub replicates_completed: usize,
    pub replicates_failed: usize,
    pub failure_reasons: Vec<String>,
    pub scenario_configuration: MultimodalScenarioConfiguration,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criterion_met_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criterion_met_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub false_positive_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub false_positive_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below_registration_resolution_flag_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below_registration_resolution_confidence_interval: Option<BinomialConfidenceInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within_margin_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within_margin_confidence_interval: Option<BinomialConfidenceInterval>,
    pub acceptance_criterion: &'static str,
    pub note: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MultimodalScenarioConfiguration {
    pub transform: &'static str,
    pub registration_max_rmse_um: f64,
    pub registration_min_landmarks: usize,
    pub radius_um: f64,
    pub label_pairs: Vec<[String; 2]>,
    pub null_models: Vec<String>,
    pub permutations: usize,
    pub cross_interaction_margin: Option<f64>,
    pub n_he_cells: usize,
    pub n_ihc_cells: usize,
    pub n_landmarks: usize,
    pub has_post: bool,
}

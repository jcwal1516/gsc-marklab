use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineCovariate {
    pub name: String,
    pub value: f64,
    pub measurement_time: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CausalUnit {
    pub unit_id: String,
    pub cluster_id: String,
    pub treatment: bool,
    pub treatment_time: f64,
    pub outcome: f64,
    pub outcome_time: f64,
    pub eligible: bool,
    pub baseline_covariates: Vec<BaselineCovariate>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterferenceEdge {
    pub left_unit: String,
    pub right_unit: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterAssignment {
    pub cluster_id: String,
    pub treated_units: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JointBinaryExposure {
    pub own_treated: bool,
    pub neighbor_any_treated: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RandomizedInterferenceSpec {
    pub design_provenance: String,
    pub graph_provenance: String,
    pub units: Vec<CausalUnit>,
    pub graph_edges: Vec<InterferenceEdge>,
    pub cluster_assignments: Vec<ClusterAssignment>,
    pub test_exposure_high: JointBinaryExposure,
    pub test_exposure_low: JointBinaryExposure,
    pub permutations: usize,
    pub seed: u64,
    pub maximum_assignment_states: u64,
    pub maximum_unit_assignment_evaluations: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObservedUnitExposure {
    pub unit_id: String,
    pub own_treated: bool,
    pub neighbor_any_treated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct JointExposureProbabilities {
    pub untreated_neighbor_untreated: f64,
    pub untreated_neighbor_treated: f64,
    pub treated_neighbor_untreated: f64,
    pub treated_neighbor_treated: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnitExposureProbabilities {
    pub unit_id: String,
    pub probabilities: JointExposureProbabilities,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExposureMeanEstimate {
    pub exposure: JointBinaryExposure,
    pub eligible_units: usize,
    pub observed_units: usize,
    pub ht_mean: Option<f64>,
    pub hajek_mean: Option<f64>,
    pub exact_fixed_outcome_ht_sd: Option<f64>,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExposureContrast {
    pub name: &'static str,
    pub high: JointBinaryExposure,
    pub low: JointBinaryExposure,
    pub estimate: Option<f64>,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct InterferenceRandomizationResult {
    pub high: JointBinaryExposure,
    pub low: JointBinaryExposure,
    pub statistic: &'static str,
    pub observed: f64,
    pub null_values: Vec<f64>,
    pub extreme_permutations: usize,
    pub p_value: f64,
    pub alternative: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RandomizedInterferenceResult {
    pub format: &'static str,
    pub version: u32,
    pub analysis_level: &'static str,
    pub null_family: &'static str,
    pub randomization_unit: &'static str,
    pub unit_count: usize,
    pub cluster_count: usize,
    pub design_provenance: String,
    pub graph_provenance: String,
    pub assignment_mechanism: &'static str,
    pub exposure_mapping: &'static str,
    pub compiled_assumptions: Vec<&'static str>,
    pub assignment_states: u64,
    pub observed_exposures: Vec<ObservedUnitExposure>,
    pub exposure_probabilities: Vec<UnitExposureProbabilities>,
    pub exposure_means: Vec<ExposureMeanEstimate>,
    pub contrasts: Vec<ExposureContrast>,
    pub randomization_test: InterferenceRandomizationResult,
    pub unit_assignment_evaluations: u64,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

#[derive(Debug, Error)]
pub enum CausalError {
    #[error("invalid causal design: {0}")]
    Invalid(String),
    #[error("causal-design resource limit exceeded: {0}")]
    Resource(String),
    #[error("causal-design numerical failure: {0}")]
    Numerical(String),
}

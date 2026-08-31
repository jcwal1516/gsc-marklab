use serde::{Deserialize, Deserializer, Serialize};
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedUnitExposure {
    pub unit_id: String,
    pub own_treated: bool,
    pub neighbor_any_treated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JointExposureProbabilities {
    pub untreated_neighbor_untreated: f64,
    pub untreated_neighbor_treated: f64,
    pub treated_neighbor_untreated: f64,
    pub treated_neighbor_treated: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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

impl<'de> Deserialize<'de> for ExposureMeanEstimate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Owned {
            exposure: JointBinaryExposure,
            eligible_units: usize,
            observed_units: usize,
            ht_mean: Option<f64>,
            hajek_mean: Option<f64>,
            exact_fixed_outcome_ht_sd: Option<f64>,
            status: String,
        }
        let owned = Owned::deserialize(deserializer)?;
        let status = match owned.status.as_str() {
            "estimated" => "estimated",
            "unavailable_no_positivity" => "unavailable_no_positivity",
            "ht_only_no_observed_hajek_denominator" => "ht_only_no_observed_hajek_denominator",
            _ => return Err(serde::de::Error::custom("unexpected exposure-mean status")),
        };
        Ok(Self {
            exposure: owned.exposure,
            eligible_units: owned.eligible_units,
            observed_units: owned.observed_units,
            ht_mean: owned.ht_mean,
            hajek_mean: owned.hajek_mean,
            exact_fixed_outcome_ht_sd: owned.exact_fixed_outcome_ht_sd,
            status,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ExposureContrast {
    pub name: &'static str,
    pub high: JointBinaryExposure,
    pub low: JointBinaryExposure,
    pub estimate: Option<f64>,
    pub status: &'static str,
}

impl<'de> Deserialize<'de> for ExposureContrast {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Owned {
            name: String,
            high: JointBinaryExposure,
            low: JointBinaryExposure,
            estimate: Option<f64>,
            status: String,
        }
        let owned = Owned::deserialize(deserializer)?;
        let name = match owned.name.as_str() {
            "direct_neighbor_untreated" => "direct_neighbor_untreated",
            "direct_neighbor_treated" => "direct_neighbor_treated",
            "spillover_untreated" => "spillover_untreated",
            "spillover_treated" => "spillover_treated",
            "total_joint" => "total_joint",
            _ => {
                return Err(serde::de::Error::custom(
                    "unexpected exposure contrast name",
                ))
            }
        };
        let status = match owned.status.as_str() {
            "estimated" => "estimated",
            "unavailable_observed_exposure_denominator" => {
                "unavailable_observed_exposure_denominator"
            }
            _ => {
                return Err(serde::de::Error::custom(
                    "unexpected exposure contrast status",
                ))
            }
        };
        Ok(Self {
            name,
            high: owned.high,
            low: owned.low,
            estimate: owned.estimate,
            status,
        })
    }
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

impl<'de> Deserialize<'de> for InterferenceRandomizationResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Owned {
            high: JointBinaryExposure,
            low: JointBinaryExposure,
            statistic: String,
            observed: f64,
            null_values: Vec<f64>,
            extreme_permutations: usize,
            p_value: f64,
            alternative: String,
        }
        let owned = Owned::deserialize(deserializer)?;
        if owned.statistic != "horvitz_thompson_exposure_mean_difference"
            || owned.alternative != "two_sided_inclusive_plus_one"
        {
            return Err(serde::de::Error::custom(
                "unexpected randomized-interference test identity",
            ));
        }
        Ok(Self {
            high: owned.high,
            low: owned.low,
            statistic: "horvitz_thompson_exposure_mean_difference",
            observed: owned.observed,
            null_values: owned.null_values,
            extreme_permutations: owned.extreme_permutations,
            p_value: owned.p_value,
            alternative: "two_sided_inclusive_plus_one",
        })
    }
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

impl<'de> Deserialize<'de> for RandomizedInterferenceResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Owned {
            format: String,
            version: u32,
            analysis_level: String,
            null_family: String,
            randomization_unit: String,
            unit_count: usize,
            cluster_count: usize,
            design_provenance: String,
            graph_provenance: String,
            assignment_mechanism: String,
            exposure_mapping: String,
            compiled_assumptions: Vec<String>,
            assignment_states: u64,
            observed_exposures: Vec<ObservedUnitExposure>,
            exposure_probabilities: Vec<UnitExposureProbabilities>,
            exposure_means: Vec<ExposureMeanEstimate>,
            contrasts: Vec<ExposureContrast>,
            randomization_test: InterferenceRandomizationResult,
            unit_assignment_evaluations: u64,
            random_seed_namespace: String,
            claim_status: String,
        }
        let owned = Owned::deserialize(deserializer)?;
        let assumptions = [
            "treatment_precedes_outcome",
            "baseline_covariates_are_pre_treatment",
            "all_units_eligible",
            "interference_graph_prespecified",
            "assignment_randomized_independently_by_cluster",
            "fixed_outcomes_under_randomization_test_null",
        ];
        if owned.format != "marklab.randomized_binary_interference"
            || owned.analysis_level != "clustered_units"
            || owned.null_family != "randomized_interference_fixed_outcomes"
            || owned.randomization_unit != "complete_cluster_assignment_state"
            || owned.assignment_mechanism != "complete_randomization_within_cluster"
            || owned.exposure_mapping != "binary_any_treated_neighbor"
            || owned.compiled_assumptions != assumptions
            || owned.random_seed_namespace != "randomized_binary_interference_v1_chacha20"
            || owned.claim_status != "randomized_design_mechanics_only"
        {
            return Err(serde::de::Error::custom(
                "unexpected randomized-interference result identity",
            ));
        }
        Ok(Self {
            format: "marklab.randomized_binary_interference",
            version: owned.version,
            analysis_level: "clustered_units",
            null_family: "randomized_interference_fixed_outcomes",
            randomization_unit: "complete_cluster_assignment_state",
            unit_count: owned.unit_count,
            cluster_count: owned.cluster_count,
            design_provenance: owned.design_provenance,
            graph_provenance: owned.graph_provenance,
            assignment_mechanism: "complete_randomization_within_cluster",
            exposure_mapping: "binary_any_treated_neighbor",
            compiled_assumptions: assumptions.into(),
            assignment_states: owned.assignment_states,
            observed_exposures: owned.observed_exposures,
            exposure_probabilities: owned.exposure_probabilities,
            exposure_means: owned.exposure_means,
            contrasts: owned.contrasts,
            randomization_test: owned.randomization_test,
            unit_assignment_evaluations: owned.unit_assignment_evaluations,
            random_seed_namespace: "randomized_binary_interference_v1_chacha20",
            claim_status: "randomized_design_mechanics_only",
        })
    }
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

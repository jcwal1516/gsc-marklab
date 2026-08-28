#![forbid(unsafe_code)]
//! Bounded causal-design research workflows for Marklab.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod eig;
mod exposure_mapping;
mod sensitivity;

pub use sensitivity::{
    binary_confounder_bias_sensitivity, manski_bounded_outcome_ate, rosenbaum_sign_sensitivity,
    BiasSensitivityResult, BiasSensitivityScenario, BiasSensitivityScenarioResult,
    BiasSensitivitySpec, BoundedOutcomeObservation, ManskiBoundedOutcomeResult,
    ManskiBoundedOutcomeSpec, MatchedPairObservation, MatchedPairSet, RosenbaumSensitivityPoint,
    RosenbaumSignSensitivityResult, RosenbaumSignSensitivitySpec,
};

pub use eig::{
    estimate_gaussian_expected_information_gain, GaussianEigOuterValue, GaussianEigResult,
    GaussianEigSpec,
};

pub use exposure_mapping::{
    compute_spatial_exposure_mapping, ExposureGraphEdge, ExposureMappingKind,
    ExposureMappingResult, ExposureMappingSpec, ExposureMappingUnit, UnitExposureValue,
};

const MAXIMUM_ASSIGNMENT_STATES: u64 = 1_000_000;
const MAXIMUM_PERMUTATIONS: usize = 1_000_000;

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

pub fn randomized_binary_interference(
    mut spec: RandomizedInterferenceSpec,
) -> Result<RandomizedInterferenceResult, CausalError> {
    spec.units.sort_by(|left, right| {
        left.cluster_id
            .cmp(&right.cluster_id)
            .then_with(|| left.unit_id.cmp(&right.unit_id))
    });
    validate_units(&spec)?;
    let unit_indices = spec
        .units
        .iter()
        .enumerate()
        .map(|(index, unit)| (unit.unit_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let adjacency = compile_graph(&spec, &unit_indices)?;
    let clusters = compile_clusters(&spec)?;
    let state_count = clusters.iter().try_fold(1_u64, |product, cluster| {
        product
            .checked_mul(combination_count(
                cluster.members.len(),
                cluster.treated_units,
            )?)
            .ok_or_else(|| CausalError::Resource("assignment-state count overflowed".into()))
    })?;
    if state_count > spec.maximum_assignment_states || state_count > MAXIMUM_ASSIGNMENT_STATES {
        return Err(CausalError::Resource(format!(
            "assignment states {state_count} exceed caller or built-in maximum"
        )));
    }
    let unit_count = spec.units.len() as u64;
    let evaluations = state_count
        .checked_mul(unit_count)
        .and_then(|value| value.checked_mul(8))
        .and_then(|value| {
            u64::try_from(spec.permutations + 1)
                .ok()
                .and_then(|replicates| replicates.checked_mul(unit_count))
                .and_then(|randomization| randomization.checked_mul(4))
                .and_then(|randomization| value.checked_add(randomization))
        })
        .ok_or_else(|| CausalError::Resource("unit-assignment work overflowed".into()))?;
    if evaluations > spec.maximum_unit_assignment_evaluations {
        return Err(CausalError::Resource(format!(
            "planned unit-operation upper bound {evaluations} exceeds declared maximum {}",
            spec.maximum_unit_assignment_evaluations
        )));
    }
    let states = enumerate_assignments(spec.units.len(), &clusters);
    if states.len() as u64 != state_count {
        return Err(CausalError::Numerical(
            "enumerated assignment-state count disagrees with exact plan".into(),
        ));
    }
    let state_exposures = states
        .iter()
        .map(|state| exposures(state, &adjacency))
        .collect::<Vec<_>>();
    let probabilities = exposure_probabilities(&state_exposures, spec.units.len());
    let observed_treatment = spec
        .units
        .iter()
        .map(|unit| unit.treatment)
        .collect::<Vec<_>>();
    let observed = exposures(&observed_treatment, &adjacency);
    let observed_exposures = spec
        .units
        .iter()
        .zip(&observed)
        .map(|(unit, exposure)| ObservedUnitExposure {
            unit_id: unit.unit_id.clone(),
            own_treated: exposure.own_treated,
            neighbor_any_treated: exposure.neighbor_any_treated,
        })
        .collect::<Vec<_>>();
    let exposure_probability_rows = spec
        .units
        .iter()
        .zip(&probabilities)
        .map(|(unit, probabilities)| UnitExposureProbabilities {
            unit_id: unit.unit_id.clone(),
            probabilities: JointExposureProbabilities {
                untreated_neighbor_untreated: probabilities[0],
                untreated_neighbor_treated: probabilities[1],
                treated_neighbor_untreated: probabilities[2],
                treated_neighbor_treated: probabilities[3],
            },
        })
        .collect::<Vec<_>>();
    let targets = [
        exposure(false, false),
        exposure(false, true),
        exposure(true, false),
        exposure(true, true),
    ];
    let exposure_means = targets
        .iter()
        .map(|&target| {
            estimate_exposure_mean(
                target,
                &spec.units,
                &observed,
                &probabilities,
                &state_exposures,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let contrasts = contrasts(&exposure_means);
    let observed_test = ht_contrast(
        spec.test_exposure_high,
        spec.test_exposure_low,
        &spec.units,
        &observed,
        &probabilities,
    )?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut null_values = Vec::with_capacity(spec.permutations);
    for _ in 0..spec.permutations {
        let state = rng.gen_range(0..states.len());
        null_values.push(ht_contrast(
            spec.test_exposure_high,
            spec.test_exposure_low,
            &spec.units,
            &state_exposures[state],
            &probabilities,
        )?);
    }
    let extreme_permutations = null_values
        .iter()
        .filter(|value| value.abs() >= observed_test.abs())
        .count();
    let p_value = (extreme_permutations + 1) as f64 / (spec.permutations + 1) as f64;
    Ok(RandomizedInterferenceResult {
        format: "marklab.randomized_binary_interference",
        version: 1,
        analysis_level: "clustered_units",
        null_family: "randomized_interference_fixed_outcomes",
        randomization_unit: "complete_cluster_assignment_state",
        unit_count: spec.units.len(),
        cluster_count: clusters.len(),
        design_provenance: spec.design_provenance,
        graph_provenance: spec.graph_provenance,
        assignment_mechanism: "complete_randomization_within_cluster",
        exposure_mapping: "binary_any_treated_neighbor",
        compiled_assumptions: vec![
            "treatment_precedes_outcome",
            "baseline_covariates_are_pre_treatment",
            "all_units_eligible",
            "interference_graph_prespecified",
            "assignment_randomized_independently_by_cluster",
            "fixed_outcomes_under_randomization_test_null",
        ],
        assignment_states: state_count,
        observed_exposures,
        exposure_probabilities: exposure_probability_rows,
        exposure_means,
        contrasts,
        randomization_test: InterferenceRandomizationResult {
            high: spec.test_exposure_high,
            low: spec.test_exposure_low,
            statistic: "horvitz_thompson_exposure_mean_difference",
            observed: observed_test,
            null_values,
            extreme_permutations,
            p_value,
            alternative: "two_sided_inclusive_plus_one",
        },
        unit_assignment_evaluations: evaluations,
        random_seed_namespace: "randomized_binary_interference_v1_chacha20",
        claim_status: "randomized_design_mechanics_only",
    })
}

struct CompiledCluster {
    members: Vec<usize>,
    treated_units: usize,
}

fn validate_units(spec: &RandomizedInterferenceSpec) -> Result<(), CausalError> {
    if spec.design_provenance.trim().is_empty()
        || spec.graph_provenance.trim().is_empty()
        || spec.units.len() < 2
        || spec.permutations == 0
        || spec.permutations > MAXIMUM_PERMUTATIONS
        || spec.test_exposure_high == spec.test_exposure_low
    {
        return Err(CausalError::Invalid(
            "provenance, distinct test exposures, at least two units, and permutations are required"
                .into(),
        ));
    }
    let mut ids = HashSet::new();
    for unit in &spec.units {
        if unit.unit_id.trim().is_empty()
            || unit.cluster_id.trim().is_empty()
            || !ids.insert(unit.unit_id.as_str())
            || !unit.treatment_time.is_finite()
            || !unit.outcome_time.is_finite()
            || unit.treatment_time >= unit.outcome_time
            || !unit.outcome.is_finite()
            || !unit.eligible
        {
            return Err(CausalError::Invalid(
                "units must have unique identities, eligibility, finite outcome/times, and treatment before outcome"
                    .into(),
            ));
        }
        let mut covariates = HashSet::new();
        for covariate in &unit.baseline_covariates {
            if covariate.name.trim().is_empty()
                || !covariates.insert(covariate.name.as_str())
                || !covariate.value.is_finite()
                || !covariate.measurement_time.is_finite()
                || covariate.measurement_time > unit.treatment_time
            {
                return Err(CausalError::Invalid(format!(
                    "unit {} has an invalid or post-treatment baseline covariate",
                    unit.unit_id
                )));
            }
        }
    }
    if spec.units.iter().all(|unit| unit.treatment) || spec.units.iter().all(|unit| !unit.treatment)
    {
        return Err(CausalError::Invalid(
            "observed treatment must vary across eligible units".into(),
        ));
    }
    Ok(())
}

fn compile_graph(
    spec: &RandomizedInterferenceSpec,
    unit_indices: &BTreeMap<String, usize>,
) -> Result<Vec<Vec<usize>>, CausalError> {
    if spec.graph_edges.is_empty() {
        return Err(CausalError::Invalid(
            "prespecified interference graph must contain an edge".into(),
        ));
    }
    let mut adjacency = vec![Vec::new(); spec.units.len()];
    let mut edges = BTreeSet::new();
    for edge in &spec.graph_edges {
        let left = unit_indices
            .get(edge.left_unit.trim())
            .copied()
            .ok_or_else(|| {
                CausalError::Invalid(format!("graph references missing unit {}", edge.left_unit))
            })?;
        let right = unit_indices
            .get(edge.right_unit.trim())
            .copied()
            .ok_or_else(|| {
                CausalError::Invalid(format!("graph references missing unit {}", edge.right_unit))
            })?;
        let key = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        if left == right || !edges.insert(key) {
            return Err(CausalError::Invalid(
                "interference edges must be unique and non-self".into(),
            ));
        }
        adjacency[left].push(right);
        adjacency[right].push(left);
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
    }
    Ok(adjacency)
}

fn compile_clusters(
    spec: &RandomizedInterferenceSpec,
) -> Result<Vec<CompiledCluster>, CausalError> {
    let mut members = BTreeMap::<String, Vec<usize>>::new();
    for (index, unit) in spec.units.iter().enumerate() {
        members
            .entry(unit.cluster_id.clone())
            .or_default()
            .push(index);
    }
    let assignments = spec
        .cluster_assignments
        .iter()
        .map(|assignment| (assignment.cluster_id.trim(), assignment.treated_units))
        .collect::<BTreeMap<_, _>>();
    if assignments.len() != spec.cluster_assignments.len()
        || assignments.len() != members.len()
        || members
            .keys()
            .any(|cluster| !assignments.contains_key(cluster.as_str()))
    {
        return Err(CausalError::Invalid(
            "cluster assignment declarations must uniquely and exactly cover unit clusters".into(),
        ));
    }
    members
        .into_iter()
        .map(|(cluster_id, members)| {
            let treated_units = assignments[cluster_id.as_str()];
            if treated_units == 0 || treated_units >= members.len() {
                return Err(CausalError::Invalid(format!(
                    "cluster {cluster_id} treated count must be strictly between zero and cluster size"
                )));
            }
            let observed = members
                .iter()
                .filter(|&&index| spec.units[index].treatment)
                .count();
            if observed != treated_units {
                return Err(CausalError::Invalid(format!(
                    "cluster {cluster_id} observed treated count {observed} differs from declared {treated_units}"
                )));
            }
            Ok(CompiledCluster {
                members,
                treated_units,
            })
        })
        .collect()
}

fn combination_count(n: usize, k: usize) -> Result<u64, CausalError> {
    let k = k.min(n - k);
    let mut result = 1_u64;
    for index in 1..=k {
        result = result
            .checked_mul((n - k + index) as u64)
            .and_then(|value| value.checked_div(index as u64))
            .ok_or_else(|| CausalError::Resource("combination count overflowed".into()))?;
    }
    Ok(result)
}

fn enumerate_assignments(unit_count: usize, clusters: &[CompiledCluster]) -> Vec<Vec<bool>> {
    let mut states = vec![vec![false; unit_count]];
    for cluster in clusters {
        let choices = combinations(&cluster.members, cluster.treated_units);
        let mut expanded = Vec::with_capacity(states.len() * choices.len());
        for state in &states {
            for choice in &choices {
                let mut next = state.clone();
                for &index in choice {
                    next[index] = true;
                }
                expanded.push(next);
            }
        }
        states = expanded;
    }
    states
}

fn combinations(values: &[usize], choose: usize) -> Vec<Vec<usize>> {
    fn visit(
        values: &[usize],
        choose: usize,
        start: usize,
        current: &mut Vec<usize>,
        result: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == choose {
            result.push(current.clone());
            return;
        }
        let needed = choose - current.len();
        for index in start..=values.len() - needed {
            current.push(values[index]);
            visit(values, choose, index + 1, current, result);
            current.pop();
        }
    }
    let mut result = Vec::new();
    visit(
        values,
        choose,
        0,
        &mut Vec::with_capacity(choose),
        &mut result,
    );
    result
}

fn exposures(treatment: &[bool], adjacency: &[Vec<usize>]) -> Vec<JointBinaryExposure> {
    treatment
        .iter()
        .enumerate()
        .map(|(index, &own_treated)| JointBinaryExposure {
            own_treated,
            neighbor_any_treated: adjacency[index].iter().any(|&neighbor| treatment[neighbor]),
        })
        .collect()
}

fn exposure_probabilities(states: &[Vec<JointBinaryExposure>], unit_count: usize) -> Vec<[f64; 4]> {
    let mut counts = vec![[0_u64; 4]; unit_count];
    for state in states {
        for (unit, exposure) in state.iter().enumerate() {
            counts[unit][exposure_index(*exposure)] += 1;
        }
    }
    counts
        .into_iter()
        .map(|counts| counts.map(|count| count as f64 / states.len() as f64))
        .collect()
}

fn estimate_exposure_mean(
    target: JointBinaryExposure,
    units: &[CausalUnit],
    observed: &[JointBinaryExposure],
    probabilities: &[[f64; 4]],
    state_exposures: &[Vec<JointBinaryExposure>],
) -> Result<ExposureMeanEstimate, CausalError> {
    let index = exposure_index(target);
    let eligible_units = probabilities.iter().filter(|row| row[index] > 0.0).count();
    if eligible_units == 0 {
        return Ok(ExposureMeanEstimate {
            exposure: target,
            eligible_units: 0,
            observed_units: 0,
            ht_mean: None,
            hajek_mean: None,
            exact_fixed_outcome_ht_sd: None,
            status: "unavailable_no_positivity",
        });
    }
    let (numerator, denominator, observed_units) =
        weighted_terms(target, units, observed, probabilities);
    let ht_mean = numerator / eligible_units as f64;
    let hajek_mean = (denominator > 0.0).then_some(numerator / denominator);
    let rerandomized = state_exposures
        .iter()
        .map(|state| {
            let (numerator, _, _) = weighted_terms(target, units, state, probabilities);
            numerator / eligible_units as f64
        })
        .collect::<Vec<_>>();
    let mean = rerandomized.iter().sum::<f64>() / rerandomized.len() as f64;
    let variance = rerandomized
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / rerandomized.len() as f64;
    if !ht_mean.is_finite() || hajek_mean.is_some_and(|value| !value.is_finite()) {
        return Err(CausalError::Numerical("exposure mean is not finite".into()));
    }
    Ok(ExposureMeanEstimate {
        exposure: target,
        eligible_units,
        observed_units,
        ht_mean: Some(ht_mean),
        hajek_mean,
        exact_fixed_outcome_ht_sd: Some(variance.sqrt()),
        status: if hajek_mean.is_some() {
            "estimated"
        } else {
            "ht_only_no_observed_hajek_denominator"
        },
    })
}

fn weighted_terms(
    target: JointBinaryExposure,
    units: &[CausalUnit],
    observed: &[JointBinaryExposure],
    probabilities: &[[f64; 4]],
) -> (f64, f64, usize) {
    let index = exposure_index(target);
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    let mut observed_units = 0;
    for unit in 0..units.len() {
        if observed[unit] == target && probabilities[unit][index] > 0.0 {
            let inverse = 1.0 / probabilities[unit][index];
            numerator += units[unit].outcome * inverse;
            denominator += inverse;
            observed_units += 1;
        }
    }
    (numerator, denominator, observed_units)
}

fn contrasts(means: &[ExposureMeanEstimate]) -> Vec<ExposureContrast> {
    [
        (
            "direct_neighbor_untreated",
            exposure(true, false),
            exposure(false, false),
        ),
        (
            "direct_neighbor_treated",
            exposure(true, true),
            exposure(false, true),
        ),
        (
            "spillover_untreated",
            exposure(false, true),
            exposure(false, false),
        ),
        (
            "spillover_treated",
            exposure(true, true),
            exposure(true, false),
        ),
        ("total_joint", exposure(true, true), exposure(false, false)),
    ]
    .into_iter()
    .map(|(name, high, low)| {
        let high_mean = means[exposure_index(high)].hajek_mean;
        let low_mean = means[exposure_index(low)].hajek_mean;
        let estimate = high_mean.zip(low_mean).map(|(high, low)| high - low);
        ExposureContrast {
            name,
            high,
            low,
            estimate,
            status: if estimate.is_some() {
                "estimated"
            } else {
                "unavailable_observed_exposure_denominator"
            },
        }
    })
    .collect()
}

fn ht_contrast(
    high: JointBinaryExposure,
    low: JointBinaryExposure,
    units: &[CausalUnit],
    observed: &[JointBinaryExposure],
    probabilities: &[[f64; 4]],
) -> Result<f64, CausalError> {
    let high_eligible = probabilities
        .iter()
        .filter(|row| row[exposure_index(high)] > 0.0)
        .count();
    let low_eligible = probabilities
        .iter()
        .filter(|row| row[exposure_index(low)] > 0.0)
        .count();
    if high_eligible == 0 || low_eligible == 0 {
        return Err(CausalError::Invalid(
            "randomization-test exposures must both satisfy positivity".into(),
        ));
    }
    let (high_numerator, _, _) = weighted_terms(high, units, observed, probabilities);
    let (low_numerator, _, _) = weighted_terms(low, units, observed, probabilities);
    let statistic = high_numerator / high_eligible as f64 - low_numerator / low_eligible as f64;
    if !statistic.is_finite() {
        return Err(CausalError::Numerical(
            "randomization-test statistic is not finite".into(),
        ));
    }
    Ok(statistic)
}

const fn exposure(own_treated: bool, neighbor_any_treated: bool) -> JointBinaryExposure {
    JointBinaryExposure {
        own_treated,
        neighbor_any_treated,
    }
}

const fn exposure_index(exposure: JointBinaryExposure) -> usize {
    exposure.own_treated as usize * 2 + exposure.neighbor_any_treated as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_cluster_randomization_states_form_cartesian_product() {
        let result = randomized_binary_interference(spec(false)).unwrap();
        assert_eq!(result.assignment_states, 4);
        assert_eq!(result.exposure_probabilities.len(), 4);
        assert_eq!(
            result.exposure_probabilities[0]
                .probabilities
                .treated_neighbor_untreated,
            0.5
        );
    }

    #[test]
    fn post_treatment_baseline_covariate_is_rejected() {
        let error = randomized_binary_interference(spec(true)).unwrap_err();
        assert!(error.to_string().contains("post-treatment"));
    }

    fn spec(post_treatment_covariate: bool) -> RandomizedInterferenceSpec {
        let mut units = vec![
            unit("a1", "a", true),
            unit("a2", "a", false),
            unit("b1", "b", true),
            unit("b2", "b", false),
        ];
        if post_treatment_covariate {
            units[0].baseline_covariates[0].measurement_time = 0.5;
        }
        RandomizedInterferenceSpec {
            design_provenance: "synthetic".into(),
            graph_provenance: "prespecified".into(),
            units,
            graph_edges: vec![
                InterferenceEdge {
                    left_unit: "a1".into(),
                    right_unit: "a2".into(),
                },
                InterferenceEdge {
                    left_unit: "b1".into(),
                    right_unit: "b2".into(),
                },
            ],
            cluster_assignments: vec![
                ClusterAssignment {
                    cluster_id: "a".into(),
                    treated_units: 1,
                },
                ClusterAssignment {
                    cluster_id: "b".into(),
                    treated_units: 1,
                },
            ],
            test_exposure_high: exposure(true, false),
            test_exposure_low: exposure(false, true),
            permutations: 10,
            seed: 7,
            maximum_assignment_states: 4,
            maximum_unit_assignment_evaluations: 10_000,
        }
    }

    fn unit(id: &str, cluster: &str, treatment: bool) -> CausalUnit {
        CausalUnit {
            unit_id: id.into(),
            cluster_id: cluster.into(),
            treatment,
            treatment_time: 0.0,
            outcome: f64::from(treatment),
            outcome_time: 1.0,
            eligible: true,
            baseline_covariates: vec![BaselineCovariate {
                name: "x".into(),
                value: 0.0,
                measurement_time: -1.0,
            }],
        }
    }
}

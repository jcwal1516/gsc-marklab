use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::{CausalError, RandomizedInterferenceSpec, MAXIMUM_PERMUTATIONS};

pub(super) struct CompiledCluster {
    pub(super) members: Vec<usize>,
    pub(super) treated_units: usize,
}

pub(super) fn validate_units(spec: &RandomizedInterferenceSpec) -> Result<(), CausalError> {
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

pub(super) fn compile_graph(
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

pub(super) fn compile_clusters(
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

pub(super) fn combination_count(n: usize, k: usize) -> Result<u64, CausalError> {
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

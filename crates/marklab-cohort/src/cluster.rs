use std::collections::{BTreeMap, HashSet};

use super::{
    numeric::{stable_mean, welch_contrast},
    CohortInferenceError, InferenceDesign, PermutationAlternative, MAXIMUM_PATIENTS,
    MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS, MAXIMUM_PERMUTATIONS,
};

const CLUSTER_PERMUTATION_NAMESPACE: u64 = 0x636c_7573_7465_725f;

#[derive(Clone, Debug)]
pub struct ClusterPatientEndpoint {
    pub patient_id: String,
    pub cluster_id: String,
    pub group: String,
    pub endpoint: f64,
}

#[derive(Clone, Debug)]
pub struct ClusterPermutationSpec {
    pub group_a: String,
    pub group_b: String,
    pub permutations: usize,
    pub seed: u64,
    pub alternative: PermutationAlternative,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClusterPermutationResult {
    pub inference_design: InferenceDesign,
    pub patient_count: usize,
    pub cluster_count: usize,
    pub group_a_cluster_count: usize,
    pub group_b_cluster_count: usize,
    pub group_a_mean: f64,
    pub group_b_mean: f64,
    pub effect_group_a_minus_group_b: f64,
    pub studentized_statistic: f64,
    pub p_value: f64,
    pub permutations_requested: usize,
    pub permutations_attempted: usize,
    pub permutations_completed: usize,
    pub seed: u64,
    pub alternative: PermutationAlternative,
}

pub fn cluster_level_permutation_test(
    records: &[ClusterPatientEndpoint],
    spec: &ClusterPermutationSpec,
) -> Result<ClusterPermutationResult, CohortInferenceError> {
    validate_spec(spec)?;
    if records.is_empty() || records.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "cluster inference requires 1 to {MAXIMUM_PATIENTS} patient rows"
        )));
    }
    let mut patients = HashSet::with_capacity(records.len());
    let mut clusters = BTreeMap::<&str, (Option<&str>, Vec<f64>)>::new();
    for record in records {
        if record.patient_id.is_empty()
            || record.patient_id.trim() != record.patient_id
            || record.cluster_id.is_empty()
            || record.cluster_id.trim() != record.cluster_id
            || !record.endpoint.is_finite()
        {
            return Err(CohortInferenceError::InvalidInput(
                "cluster rows require exact non-empty IDs and finite endpoints".into(),
            ));
        }
        if !patients.insert(record.patient_id.as_str()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate cluster patient_id: {}",
                record.patient_id
            )));
        }
        if record.group != spec.group_a && record.group != spec.group_b {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has undeclared group {:?}",
                record.patient_id, record.group
            )));
        }
        let entry = clusters
            .entry(record.cluster_id.as_str())
            .or_insert((None, Vec::new()));
        if entry.0.is_some_and(|group| group != record.group) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "cluster {} contains conflicting group labels",
                record.cluster_id
            )));
        }
        entry.0 = Some(record.group.as_str());
        entry.1.push(record.endpoint);
    }
    let cluster_count = clusters.len();
    let work = cluster_count
        .checked_mul(spec.permutations)
        .ok_or_else(work_limit_error)?;
    if work > MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS {
        return Err(work_limit_error());
    }
    let mut values = Vec::with_capacity(cluster_count);
    let mut labels = Vec::with_capacity(cluster_count);
    for (group, endpoints) in clusters.into_values() {
        values.push(stable_mean(&endpoints)?);
        labels.push(group == Some(spec.group_a.as_str()));
    }
    let observed = welch_contrast(&values, &labels)?;
    let design = InferenceDesign::cluster_label_permutation(
        cluster_count,
        spec.permutations,
        spec.seed,
        CLUSTER_PERMUTATION_NAMESPACE,
        spec.alternative,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
    let mut lower_tail = 0usize;
    let mut upper_tail = 0usize;
    for replicate in 0..spec.permutations {
        let permuted = design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
            .iter()
            .map(|source| labels[*source])
            .collect::<Vec<_>>();
        let statistic = welch_contrast(&values, &permuted)?.studentized;
        lower_tail += usize::from(statistic <= observed.studentized);
        upper_tail += usize::from(statistic >= observed.studentized);
    }
    let denominator = (spec.permutations + 1) as f64;
    let p_value = match spec.alternative {
        PermutationAlternative::Less => (lower_tail as f64 + 1.0) / denominator,
        PermutationAlternative::Greater => (upper_tail as f64 + 1.0) / denominator,
        PermutationAlternative::TwoSided => {
            (2.0 * ((lower_tail.min(upper_tail) as f64 + 1.0) / denominator)).min(1.0)
        }
    };
    Ok(ClusterPermutationResult {
        inference_design: design,
        patient_count: records.len(),
        cluster_count,
        group_a_cluster_count: observed.group_a_count,
        group_b_cluster_count: observed.group_b_count,
        group_a_mean: observed.group_a_mean,
        group_b_mean: observed.group_b_mean,
        effect_group_a_minus_group_b: observed.effect,
        studentized_statistic: observed.studentized,
        p_value,
        permutations_requested: spec.permutations,
        permutations_attempted: spec.permutations,
        permutations_completed: spec.permutations,
        seed: spec.seed,
        alternative: spec.alternative,
    })
}

fn validate_spec(spec: &ClusterPermutationSpec) -> Result<(), CohortInferenceError> {
    if spec.group_a.is_empty()
        || spec.group_b.is_empty()
        || spec.group_a.trim() != spec.group_a
        || spec.group_b.trim() != spec.group_b
        || spec.group_a == spec.group_b
    {
        return Err(CohortInferenceError::InvalidInput(
            "cluster groups must be distinct exact non-empty labels".into(),
        ));
    }
    if spec.permutations == 0 || spec.permutations > MAXIMUM_PERMUTATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "permutations must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )));
    }
    Ok(())
}

fn work_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "cluster-by-permutation work exceeds the {MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS}-evaluation limit"
    ))
}

use std::collections::{BTreeMap, HashSet};

use super::{
    max_t_multiple_endpoint_step_down_permutation, CohortInferenceError, MaxTPermutationResult,
    MaxTPermutationSpec, PatientEndpointVector,
};

const FIXED_MAXIMUM_PATIENTS: usize = 100_000;
const FIXED_MAXIMUM_SPECIMENS: usize = 1_000_000;
const FIXED_MAXIMUM_ENDPOINTS: usize = 4_096;
const FIXED_MAXIMUM_EVALUATIONS: u64 = 100_000_000;

/// One finite field endpoint measured on a specimen nested within one patient.
#[derive(Clone, Debug)]
pub struct NestedFieldRecord {
    pub patient_id: String,
    pub specimen_id: String,
    pub group: String,
    pub endpoint: String,
    pub value: f64,
}

/// Frozen inference and resource controls for patient-nested field reduction.
#[derive(Clone, Debug, PartialEq)]
pub struct NestedFieldSpec {
    pub group_a: String,
    pub group_b: String,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub maximum_patients: usize,
    pub maximum_specimens: usize,
    pub maximum_endpoints: usize,
    pub maximum_permutation_endpoint_evaluations: u64,
    pub memory_budget_bytes: usize,
}

/// One patient-level endpoint vector after equal-weight reduction over specimens.
#[derive(Clone, Debug, PartialEq)]
pub struct NestedFieldPatient {
    pub patient_id: String,
    pub group: String,
    pub specimen_count: usize,
    pub values: Vec<f64>,
}

/// Stability of patient means to retaining one nested specimen at a time.
#[derive(Clone, Debug, PartialEq)]
pub struct NestedSpecimenStability {
    pub method: &'static str,
    pub comparison_count: usize,
    pub median_rank_correlation: f64,
    pub minimum_rank_correlation: f64,
}

/// One leave-one-patient-out prediction made without fitting on that patient.
#[derive(Clone, Debug, PartialEq)]
pub struct NestedFieldHeldoutPrediction {
    pub patient_id: String,
    pub observed_group: String,
    pub predicted_group: String,
    pub distance_to_group_a: f64,
    pub distance_to_group_b: f64,
}

/// Patient-held-out nearest-centroid assessment with fold-internal transforms.
#[derive(Clone, Debug, PartialEq)]
pub struct NestedFieldHeldoutResult {
    pub split_unit: &'static str,
    pub training_transform: &'static str,
    pub classifier: &'static str,
    pub balanced_accuracy: f64,
    pub predictions: Vec<NestedFieldHeldoutPrediction>,
}

/// Complete patient-level result for a prespecified slide-field endpoint family.
#[derive(Clone, Debug, PartialEq)]
pub struct NestedFieldResult {
    pub endpoints: Vec<String>,
    pub patients: Vec<NestedFieldPatient>,
    pub specimen_count: usize,
    pub stability: NestedSpecimenStability,
    pub heldout: NestedFieldHeldoutResult,
    pub population_inference: MaxTPermutationResult,
    pub retained_memory_bytes: usize,
    pub permutation_endpoint_evaluations: u64,
}

/// Validated and reduced patient-nested field data before inferential execution.
#[derive(Clone, Debug)]
pub struct PreparedNestedFields {
    spec: NestedFieldSpec,
    endpoints: Vec<String>,
    patients: Vec<NestedFieldPatient>,
    nested_specimens: Vec<Vec<Vec<f64>>>,
    specimen_count: usize,
    retained_memory_bytes: usize,
    permutation_endpoint_evaluations: u64,
}

impl PreparedNestedFields {
    pub fn patient_count(&self) -> usize {
        self.patients.len()
    }

    pub fn specimen_count(&self) -> usize {
        self.specimen_count
    }

    pub fn endpoint_count(&self) -> usize {
        self.endpoints.len()
    }

    pub fn permutation_endpoint_evaluations(&self) -> u64 {
        self.permutation_endpoint_evaluations
    }
}

#[derive(Default)]
struct Specimen {
    group: String,
    values: BTreeMap<String, f64>,
}

/// Reduce complete specimen endpoint vectors inside patients and perform patient-unit inference.
pub fn patient_nested_field_inference(
    records: &[NestedFieldRecord],
    spec: &NestedFieldSpec,
) -> Result<NestedFieldResult, CohortInferenceError> {
    let prepared = prepare_patient_nested_fields(records, spec)?;
    execute_patient_nested_fields(&prepared, spec)
}

/// Validate resources and reduce complete specimen vectors without running permutations.
pub fn prepare_patient_nested_fields(
    records: &[NestedFieldRecord],
    spec: &NestedFieldSpec,
) -> Result<PreparedNestedFields, CohortInferenceError> {
    validate_spec(spec)?;
    if records.is_empty() {
        return invalid("patient-nested field input is empty");
    }
    let mut specimens = BTreeMap::<(String, String), Specimen>::new();
    let mut endpoint_names = HashSet::<String>::new();
    for record in records {
        validate_exact_id(&record.patient_id, "patient_id")?;
        validate_exact_id(&record.specimen_id, "specimen_id")?;
        validate_exact_id(&record.group, "group")?;
        validate_exact_id(&record.endpoint, "endpoint")?;
        if record.group != spec.group_a && record.group != spec.group_b {
            return invalid(format!(
                "patient {} has undeclared group {}",
                record.patient_id, record.group
            ));
        }
        if !record.value.is_finite() {
            return invalid(format!(
                "patient {}/specimen {}/endpoint {} is non-finite",
                record.patient_id, record.specimen_id, record.endpoint
            ));
        }
        endpoint_names.insert(record.endpoint.clone());
        let specimen = specimens
            .entry((record.patient_id.clone(), record.specimen_id.clone()))
            .or_default();
        if specimen.group.is_empty() {
            specimen.group = record.group.clone();
        } else if specimen.group != record.group {
            return invalid(format!(
                "patient {}/specimen {} has conflicting groups",
                record.patient_id, record.specimen_id
            ));
        }
        if specimen
            .values
            .insert(record.endpoint.clone(), record.value)
            .is_some()
        {
            return invalid(format!(
                "patient {}/specimen {} has duplicate endpoint {}",
                record.patient_id, record.specimen_id, record.endpoint
            ));
        }
    }
    if specimens.len() > spec.maximum_specimens {
        return invalid(format!(
            "specimen count {} exceeds maximum_specimens {}",
            specimens.len(),
            spec.maximum_specimens
        ));
    }
    let mut endpoints = endpoint_names.into_iter().collect::<Vec<_>>();
    endpoints.sort();
    if endpoints.len() < 2 || endpoints.len() > spec.maximum_endpoints {
        return invalid(format!(
            "endpoint count {} must be between 2 and maximum_endpoints {}",
            endpoints.len(),
            spec.maximum_endpoints
        ));
    }
    for ((patient, specimen_id), specimen) in &specimens {
        if specimen.values.len() != endpoints.len()
            || endpoints
                .iter()
                .any(|endpoint| !specimen.values.contains_key(endpoint))
        {
            return invalid(format!(
                "patient {patient}/specimen {specimen_id} does not contain the exact complete endpoint vector"
            ));
        }
    }

    let retained_memory_bytes = estimate_memory(records, specimens.len(), endpoints.len())?;
    if retained_memory_bytes > spec.memory_budget_bytes {
        return invalid(format!(
            "retained-memory estimate {retained_memory_bytes} exceeds budget {}",
            spec.memory_budget_bytes
        ));
    }

    let mut nested = BTreeMap::<String, (String, Vec<Vec<f64>>)>::new();
    for ((patient, _), specimen) in &specimens {
        let entry = nested
            .entry(patient.clone())
            .or_insert_with(|| (specimen.group.clone(), Vec::new()));
        if entry.0 != specimen.group {
            return invalid(format!("patient {patient} has conflicting group labels"));
        }
        entry.1.push(
            endpoints
                .iter()
                .map(|endpoint| specimen.values[endpoint])
                .collect(),
        );
    }
    if nested.len() > spec.maximum_patients {
        return invalid(format!(
            "patient count {} exceeds maximum_patients {}",
            nested.len(),
            spec.maximum_patients
        ));
    }
    if nested.values().any(|(_, rows)| rows.len() < 2) {
        return invalid("every patient requires at least two nested specimens");
    }

    let patients = nested
        .iter()
        .map(|(patient_id, (group, rows))| NestedFieldPatient {
            patient_id: patient_id.clone(),
            group: group.clone(),
            specimen_count: rows.len(),
            values: column_means(rows),
        })
        .collect::<Vec<_>>();
    let permutation_endpoint_evaluations = (patients.len() as u64)
        .checked_mul(endpoints.len() as u64)
        .and_then(|value| value.checked_mul((spec.permutations + 1) as u64))
        .ok_or_else(|| CohortInferenceError::InvalidInput("permutation work overflowed".into()))?;
    if permutation_endpoint_evaluations > spec.maximum_permutation_endpoint_evaluations {
        return invalid(format!(
            "permutation endpoint evaluations {permutation_endpoint_evaluations} exceed maximum_permutation_endpoint_evaluations {}",
            spec.maximum_permutation_endpoint_evaluations
        ));
    }
    let nested_specimens = nested
        .values()
        .map(|(_, rows)| rows.clone())
        .collect::<Vec<_>>();
    Ok(PreparedNestedFields {
        spec: spec.clone(),
        endpoints,
        patients,
        nested_specimens,
        specimen_count: specimens.len(),
        retained_memory_bytes,
        permutation_endpoint_evaluations,
    })
}

/// Execute held-out, stability, and whole-patient inference on validated preparation.
pub fn execute_patient_nested_fields(
    prepared: &PreparedNestedFields,
    spec: &NestedFieldSpec,
) -> Result<NestedFieldResult, CohortInferenceError> {
    if &prepared.spec != spec {
        return invalid("patient-nested field preparation and execution specifications differ");
    }
    let max_t_patients = prepared
        .patients
        .iter()
        .map(|patient| PatientEndpointVector {
            patient_id: patient.patient_id.clone(),
            group: patient.group.clone(),
            endpoints: prepared.endpoints.clone(),
            values: patient.values.clone(),
        })
        .collect::<Vec<_>>();
    let population_inference = max_t_multiple_endpoint_step_down_permutation(
        &max_t_patients,
        &MaxTPermutationSpec {
            group_a: spec.group_a.clone(),
            group_b: spec.group_b.clone(),
            permutations: spec.permutations,
            seed: spec.seed,
            alpha: spec.alpha,
        },
    )?;
    let stability = stability(&prepared.nested_specimens)?;
    let heldout = heldout(&prepared.patients, &spec.group_a, &spec.group_b)?;
    Ok(NestedFieldResult {
        endpoints: prepared.endpoints.clone(),
        patients: prepared.patients.clone(),
        specimen_count: prepared.specimen_count,
        stability,
        heldout,
        population_inference,
        retained_memory_bytes: prepared.retained_memory_bytes,
        permutation_endpoint_evaluations: prepared.permutation_endpoint_evaluations,
    })
}

fn validate_spec(spec: &NestedFieldSpec) -> Result<(), CohortInferenceError> {
    validate_exact_id(&spec.group_a, "group_a")?;
    validate_exact_id(&spec.group_b, "group_b")?;
    if spec.group_a == spec.group_b {
        return invalid("group_a and group_b must differ");
    }
    if spec.maximum_patients == 0
        || spec.maximum_patients > FIXED_MAXIMUM_PATIENTS
        || spec.maximum_specimens == 0
        || spec.maximum_specimens > FIXED_MAXIMUM_SPECIMENS
        || spec.maximum_endpoints < 2
        || spec.maximum_endpoints > FIXED_MAXIMUM_ENDPOINTS
        || spec.maximum_permutation_endpoint_evaluations == 0
        || spec.maximum_permutation_endpoint_evaluations > FIXED_MAXIMUM_EVALUATIONS
        || spec.memory_budget_bytes == 0
    {
        return invalid(
            "patient-nested field resource limits are outside fixed production ceilings",
        );
    }
    Ok(())
}

fn validate_exact_id(value: &str, label: &str) -> Result<(), CohortInferenceError> {
    if value.is_empty() || value.trim() != value {
        return invalid(format!("{label} must be exact and nonempty"));
    }
    Ok(())
}

fn column_means(rows: &[Vec<f64>]) -> Vec<f64> {
    (0..rows[0].len())
        .map(|column| rows.iter().map(|row| row[column]).sum::<f64>() / rows.len() as f64)
        .collect()
}

fn stability(nested: &[Vec<Vec<f64>>]) -> Result<NestedSpecimenStability, CohortInferenceError> {
    let mut correlations = Vec::new();
    for rows in nested {
        let patient_mean = column_means(rows);
        for row in rows {
            correlations.push(spearman(&patient_mean, row)?);
        }
    }
    correlations.sort_by(f64::total_cmp);
    let middle = correlations.len() / 2;
    let median = if correlations.len() % 2 == 0 {
        (correlations[middle - 1] + correlations[middle]) / 2.0
    } else {
        correlations[middle]
    };
    Ok(NestedSpecimenStability {
        method: "specimen_vs_equal_weight_patient_mean_rank_spearman",
        comparison_count: correlations.len(),
        median_rank_correlation: median,
        minimum_rank_correlation: correlations[0],
    })
}

fn spearman(left: &[f64], right: &[f64]) -> Result<f64, CohortInferenceError> {
    let left = ranks(left);
    let right = ranks(right);
    let left_mean = left.iter().sum::<f64>() / left.len() as f64;
    let right_mean = right.iter().sum::<f64>() / right.len() as f64;
    let covariance = left
        .iter()
        .zip(&right)
        .map(|(a, b)| (a - left_mean) * (b - right_mean))
        .sum::<f64>();
    let left_ss = left
        .iter()
        .map(|value| (value - left_mean).powi(2))
        .sum::<f64>();
    let right_ss = right
        .iter()
        .map(|value| (value - right_mean).powi(2))
        .sum::<f64>();
    if left_ss == 0.0 || right_ss == 0.0 {
        return Err(CohortInferenceError::NumericalFailure(
            "nested specimen stability is undefined for a constant endpoint rank vector".into(),
        ));
    }
    Ok(covariance / (left_ss * right_ss).sqrt())
}

fn ranks(values: &[f64]) -> Vec<f64> {
    let mut order = (0..values.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| values[*left].total_cmp(&values[*right]));
    let mut ranks = vec![0.0; values.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && values[order[start]].to_bits() == values[order[end]].to_bits() {
            end += 1;
        }
        let rank = (start + end - 1) as f64 / 2.0 + 1.0;
        for index in &order[start..end] {
            ranks[*index] = rank;
        }
        start = end;
    }
    ranks
}

fn heldout(
    patients: &[NestedFieldPatient],
    group_a: &str,
    group_b: &str,
) -> Result<NestedFieldHeldoutResult, CohortInferenceError> {
    let count_a = patients
        .iter()
        .filter(|patient| patient.group == group_a)
        .count();
    let count_b = patients.len() - count_a;
    if count_a < 2 || count_b < 2 {
        return invalid("patient-held-out inference requires at least two patients per group");
    }
    let mut predictions = Vec::with_capacity(patients.len());
    for heldout in patients {
        let training = patients
            .iter()
            .filter(|patient| patient.patient_id != heldout.patient_id)
            .collect::<Vec<_>>();
        let dimension = heldout.values.len();
        let means = (0..dimension)
            .map(|column| {
                training
                    .iter()
                    .map(|patient| patient.values[column])
                    .sum::<f64>()
                    / training.len() as f64
            })
            .collect::<Vec<_>>();
        let scales = (0..dimension)
            .map(|column| {
                let variance = training
                    .iter()
                    .map(|patient| (patient.values[column] - means[column]).powi(2))
                    .sum::<f64>()
                    / (training.len() - 1) as f64;
                variance.sqrt()
            })
            .collect::<Vec<_>>();
        if scales
            .iter()
            .any(|scale| !scale.is_finite() || *scale == 0.0)
        {
            return Err(CohortInferenceError::NumericalFailure(
                "a training fold has a zero-variance field endpoint".into(),
            ));
        }
        let transform = |values: &[f64]| {
            values
                .iter()
                .enumerate()
                .map(|(column, value)| (value - means[column]) / scales[column])
                .collect::<Vec<_>>()
        };
        let transformed_training = training
            .iter()
            .map(|patient| (patient.group.as_str(), transform(&patient.values)))
            .collect::<Vec<_>>();
        let centroid = |group: &str| {
            let rows = transformed_training
                .iter()
                .filter(|(label, _)| *label == group)
                .map(|(_, values)| values.clone())
                .collect::<Vec<_>>();
            column_means(&rows)
        };
        let centroid_a = centroid(group_a);
        let centroid_b = centroid(group_b);
        let observed = transform(&heldout.values);
        let distance = |centroid: &[f64]| {
            observed
                .iter()
                .zip(centroid)
                .map(|(value, center)| (value - center).powi(2))
                .sum::<f64>()
                .sqrt()
        };
        let distance_to_group_a = distance(&centroid_a);
        let distance_to_group_b = distance(&centroid_b);
        let predicted_group = if distance_to_group_a <= distance_to_group_b {
            group_a
        } else {
            group_b
        };
        predictions.push(NestedFieldHeldoutPrediction {
            patient_id: heldout.patient_id.clone(),
            observed_group: heldout.group.clone(),
            predicted_group: predicted_group.into(),
            distance_to_group_a,
            distance_to_group_b,
        });
    }
    let sensitivity = |group: &str| {
        let rows = predictions
            .iter()
            .filter(|prediction| prediction.observed_group == group)
            .collect::<Vec<_>>();
        rows.iter()
            .filter(|prediction| prediction.predicted_group == group)
            .count() as f64
            / rows.len() as f64
    };
    Ok(NestedFieldHeldoutResult {
        split_unit: "whole_patient",
        training_transform: "fold_internal_z_score",
        classifier: "nearest_group_centroid_euclidean",
        balanced_accuracy: (sensitivity(group_a) + sensitivity(group_b)) / 2.0,
        predictions,
    })
}

fn estimate_memory(
    records: &[NestedFieldRecord],
    specimen_count: usize,
    endpoint_count: usize,
) -> Result<usize, CohortInferenceError> {
    let string_bytes = records.iter().try_fold(0usize, |sum, record| {
        sum.checked_add(record.patient_id.len())
            .and_then(|value| value.checked_add(record.specimen_id.len()))
            .and_then(|value| value.checked_add(record.group.len()))
            .and_then(|value| value.checked_add(record.endpoint.len()))
    });
    string_bytes
        .and_then(|bytes| bytes.checked_add(records.len() * 64))
        .and_then(|bytes| bytes.checked_add(specimen_count * endpoint_count * 16))
        .ok_or_else(|| CohortInferenceError::InvalidInput("memory estimate overflowed".into()))
}

fn invalid<T>(message: impl Into<String>) -> Result<T, CohortInferenceError> {
    Err(CohortInferenceError::InvalidInput(message.into()))
}

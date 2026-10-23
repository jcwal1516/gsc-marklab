use std::collections::{BTreeMap, BTreeSet};

use super::{
    ols::OlsContext, CohortInferenceError, InferenceDesign,
    MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS, MAXIMUM_PERMUTATIONS,
};

const REPEATED_FREEDMAN_LANE_NAMESPACE: u64 = 0x7265_7065_6174_666c;

#[derive(Clone, Debug)]
pub struct RepeatedMeasureRecord {
    pub subject_id: String,
    pub visit_id: String,
    pub outcome: f64,
    pub target: f64,
}

#[derive(Clone, Debug)]
pub struct RepeatedFreedmanLaneSpec {
    pub permutations: usize,
    pub seed: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RepeatedFreedmanLaneResult {
    pub inference_design: InferenceDesign,
    pub subject_count: usize,
    pub row_count: usize,
    pub full_model_columns: usize,
    pub reduced_model_columns: usize,
    pub residual_degrees_of_freedom: usize,
    pub target_coefficient: f64,
    pub target_standard_error: f64,
    pub studentized_statistic: f64,
    pub p_value: f64,
    pub permutations_requested: usize,
    pub seed: u64,
}

pub fn repeated_measures_freedman_lane(
    records: &[RepeatedMeasureRecord],
    spec: &RepeatedFreedmanLaneSpec,
) -> Result<RepeatedFreedmanLaneResult, CohortInferenceError> {
    if spec.permutations == 0 || spec.permutations > MAXIMUM_PERMUTATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "permutations must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )));
    }
    let rows = canonicalize(records)?;
    let subject_count = rows
        .iter()
        .map(|row| row.subject_index)
        .max()
        .map_or(0, |value| value + 1);
    let work = rows.len().checked_mul(spec.permutations).ok_or_else(|| {
        CohortInferenceError::InvalidInput(
            "repeated-row permutation work exceeds the supported limit".into(),
        )
    })?;
    if work > MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "repeated-row permutation work exceeds the {MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS}-evaluation limit"
        )));
    }
    let reduced = design_matrix(&rows, subject_count, false);
    let full = design_matrix(&rows, subject_count, true);
    let outcomes = rows.iter().map(|row| row.outcome).collect::<Vec<_>>();
    let reduced_fit = OlsContext::Repeated.fit(&reduced, &outcomes, None, 0)?;
    let observed = OlsContext::Repeated.fit(&full, &outcomes, Some(subject_count), 0)?;
    let residuals = outcomes
        .iter()
        .zip(&reduced_fit.fitted)
        .map(|(outcome, fitted)| outcome - fitted)
        .collect::<Vec<_>>();
    let inference_design = InferenceDesign::subject_residual_sign_symmetry(
        subject_count,
        spec.permutations,
        spec.seed,
        REPEATED_FREEDMAN_LANE_NAMESPACE,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;

    let mut upper_tail = 0_usize;
    let mut permuted_outcomes = vec![0.0; rows.len()];
    for replicate in 0..spec.permutations {
        let signs = inference_design
            .subject_residual_signs(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
        for (index, row) in rows.iter().enumerate() {
            permuted_outcomes[index] =
                reduced_fit.fitted[index] + f64::from(signs[row.subject_index]) * residuals[index];
        }
        let statistic = OlsContext::Repeated
            .fit(&full, &permuted_outcomes, Some(subject_count), 0)?
            .target_statistic;
        upper_tail += usize::from(statistic.abs() >= observed.target_statistic.abs());
    }
    let p_value = (upper_tail as f64 + 1.0) / (spec.permutations + 1) as f64;
    Ok(RepeatedFreedmanLaneResult {
        inference_design,
        subject_count,
        row_count: rows.len(),
        full_model_columns: full[0].len(),
        reduced_model_columns: reduced[0].len(),
        residual_degrees_of_freedom: observed.residual_degrees_of_freedom,
        target_coefficient: observed.target_coefficient,
        target_standard_error: observed.target_standard_error,
        studentized_statistic: observed.target_statistic,
        p_value,
        permutations_requested: spec.permutations,
        seed: spec.seed,
    })
}

#[derive(Clone, Copy)]
struct Row {
    subject_index: usize,
    outcome: f64,
    target: f64,
}

fn canonicalize(records: &[RepeatedMeasureRecord]) -> Result<Vec<Row>, CohortInferenceError> {
    if records.len() < 8 {
        return Err(CohortInferenceError::InvalidInput(
            "repeated inference requires at least eight rows".into(),
        ));
    }
    let mut grouped = BTreeMap::<&str, BTreeMap<&str, (f64, f64)>>::new();
    for record in records {
        if record.subject_id.is_empty()
            || record.subject_id.trim() != record.subject_id
            || record.visit_id.is_empty()
            || record.visit_id.trim() != record.visit_id
            || !record.outcome.is_finite()
            || !record.target.is_finite()
        {
            return Err(CohortInferenceError::InvalidInput(
                "repeated rows require exact non-empty IDs and finite outcome/target".into(),
            ));
        }
        if grouped
            .entry(record.subject_id.as_str())
            .or_default()
            .insert(record.visit_id.as_str(), (record.outcome, record.target))
            .is_some()
        {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate subject/visit row: {}/{}",
                record.subject_id, record.visit_id
            )));
        }
    }
    if grouped.len() < 4 || grouped.values().any(|visits| visits.len() < 2) {
        return Err(CohortInferenceError::InvalidInput(
            "repeated inference requires at least four subjects with two visits each".into(),
        ));
    }
    let mut rows = Vec::with_capacity(records.len());
    for (subject_index, visits) in grouped.into_values().enumerate() {
        for (outcome, target) in visits.into_values() {
            rows.push(Row {
                subject_index,
                outcome,
                target,
            });
        }
    }
    let target_values = rows
        .iter()
        .map(|row| row.target.to_bits())
        .collect::<BTreeSet<_>>();
    if target_values.len() < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "target column must vary".into(),
        ));
    }
    Ok(rows)
}

fn design_matrix(rows: &[Row], subject_count: usize, target: bool) -> Vec<Vec<f64>> {
    rows.iter()
        .map(|row| {
            let mut values = vec![0.0; subject_count + usize::from(target)];
            values[0] = 1.0;
            if row.subject_index > 0 {
                values[row.subject_index] = 1.0;
            }
            if target {
                values[subject_count] = row.target;
            }
            values
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_design_rejects_duplicate_subject_visit() {
        let mut records = (0..4)
            .flat_map(|subject| {
                (0..2).map(move |visit| RepeatedMeasureRecord {
                    subject_id: format!("s-{subject}"),
                    visit_id: format!("v-{visit}"),
                    outcome: (subject + visit) as f64,
                    target: visit as f64,
                })
            })
            .collect::<Vec<_>>();
        records.push(records[0].clone());
        let result = repeated_measures_freedman_lane(
            &records,
            &RepeatedFreedmanLaneSpec {
                permutations: 9,
                seed: 1,
            },
        );
        assert!(matches!(result, Err(CohortInferenceError::InvalidInput(_))));
    }
}

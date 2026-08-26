use std::collections::{BTreeMap, BTreeSet};

use super::{
    derive_seed_in_namespace, splitmix64, CohortInferenceError,
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
    pub permutations_completed: usize,
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
    let reduced_fit = fit_ols(&reduced, &outcomes, false)?;
    let observed = fit_ols(&full, &outcomes, true)?;
    let residuals = outcomes
        .iter()
        .zip(&reduced_fit.fitted)
        .map(|(outcome, fitted)| outcome - fitted)
        .collect::<Vec<_>>();

    let mut upper_tail = 0_usize;
    let mut permuted_outcomes = vec![0.0; rows.len()];
    for replicate in 0..spec.permutations {
        let mut state =
            derive_seed_in_namespace(spec.seed, REPEATED_FREEDMAN_LANE_NAMESPACE, replicate);
        let mut signs = vec![1.0; subject_count];
        for (subject, sign) in signs.iter_mut().enumerate() {
            state = splitmix64(state ^ subject as u64);
            *sign = if state & 1 == 0 { 1.0 } else { -1.0 };
        }
        for (index, row) in rows.iter().enumerate() {
            permuted_outcomes[index] =
                reduced_fit.fitted[index] + signs[row.subject_index] * residuals[index];
        }
        let statistic = fit_ols(&full, &permuted_outcomes, true)?.target_statistic;
        upper_tail += usize::from(statistic.abs() >= observed.target_statistic.abs());
    }
    let p_value = (upper_tail as f64 + 1.0) / (spec.permutations + 1) as f64;
    Ok(RepeatedFreedmanLaneResult {
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
        permutations_completed: spec.permutations,
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

struct OlsFit {
    fitted: Vec<f64>,
    target_coefficient: f64,
    target_standard_error: f64,
    target_statistic: f64,
    residual_degrees_of_freedom: usize,
}

fn fit_ols(
    design: &[Vec<f64>],
    outcome: &[f64],
    require_target: bool,
) -> Result<OlsFit, CohortInferenceError> {
    let row_count = design.len();
    let column_count = design[0].len();
    if row_count <= column_count {
        return Err(CohortInferenceError::InvalidInput(
            "repeated full model has no residual degrees of freedom".into(),
        ));
    }
    let mut cross = vec![vec![0.0; column_count]; column_count];
    let mut rhs = vec![0.0; column_count];
    for (row, value) in design.iter().zip(outcome) {
        for left in 0..column_count {
            rhs[left] += row[left] * value;
            for right in 0..column_count {
                cross[left][right] += row[left] * row[right];
            }
        }
    }
    let coefficients = solve(&cross, &rhs)?;
    let fitted = design
        .iter()
        .map(|row| {
            row.iter()
                .zip(&coefficients)
                .map(|(x, beta)| x * beta)
                .sum()
        })
        .collect::<Vec<f64>>();
    let residual_degrees_of_freedom = row_count - column_count;
    if !require_target {
        return Ok(OlsFit {
            fitted,
            target_coefficient: 0.0,
            target_standard_error: 0.0,
            target_statistic: 0.0,
            residual_degrees_of_freedom,
        });
    }
    let rss = outcome
        .iter()
        .zip(&fitted)
        .map(|(actual, predicted)| (actual - predicted).powi(2))
        .sum::<f64>();
    let variance = rss / residual_degrees_of_freedom as f64;
    let target_index = column_count - 1;
    let mut unit = vec![0.0; column_count];
    unit[target_index] = 1.0;
    let inverse_column = solve(&cross, &unit)?;
    let standard_error = (variance * inverse_column[target_index]).sqrt();
    let coefficient = coefficients[target_index];
    let statistic = coefficient / standard_error;
    if !coefficient.is_finite()
        || !standard_error.is_finite()
        || standard_error <= 0.0
        || !statistic.is_finite()
    {
        return Err(CohortInferenceError::NumericalFailure(
            "repeated target statistic is undefined or non-finite".into(),
        ));
    }
    Ok(OlsFit {
        fitted,
        target_coefficient: coefficient,
        target_standard_error: standard_error,
        target_statistic: statistic,
        residual_degrees_of_freedom,
    })
}

fn solve(matrix: &[Vec<f64>], rhs: &[f64]) -> Result<Vec<f64>, CohortInferenceError> {
    let size = rhs.len();
    let mut augmented = matrix
        .iter()
        .zip(rhs)
        .map(|(row, value)| {
            let mut row = row.clone();
            row.push(*value);
            row
        })
        .collect::<Vec<_>>();
    for pivot in 0..size {
        let selected = (pivot..size)
            .max_by(|left, right| {
                augmented[*left][pivot]
                    .abs()
                    .total_cmp(&augmented[*right][pivot].abs())
            })
            .expect("non-empty pivot range");
        if augmented[selected][pivot].abs() <= 1e-12 {
            return Err(CohortInferenceError::InvalidInput(
                "repeated model design is rank deficient".into(),
            ));
        }
        augmented.swap(pivot, selected);
        let diagonal = augmented[pivot][pivot];
        for value in &mut augmented[pivot][pivot..=size] {
            *value /= diagonal;
        }
        for row in 0..size {
            if row == pivot {
                continue;
            }
            let factor = augmented[row][pivot];
            let pivot_values = augmented[pivot][pivot..=size].to_vec();
            for (value, pivot_value) in augmented[row][pivot..=size].iter_mut().zip(pivot_values) {
                *value -= factor * pivot_value;
            }
        }
    }
    let solution = augmented.iter().map(|row| row[size]).collect::<Vec<_>>();
    if solution.iter().any(|value| !value.is_finite()) {
        return Err(CohortInferenceError::NumericalFailure(
            "repeated model solve produced a non-finite coefficient".into(),
        ));
    }
    Ok(solution)
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

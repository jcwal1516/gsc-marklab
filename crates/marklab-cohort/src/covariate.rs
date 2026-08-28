use std::collections::BTreeMap;

use super::{
    inference_design::align_patient_blocks, numeric::stable_mean, CohortInferenceError,
    InferenceAlternative, InferenceDesign, PatientExchangeabilityBlock, PermutationAlternative,
    MAXIMUM_PATIENTS, MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS, MAXIMUM_PERMUTATIONS,
};

const COVARIATE_FREEDMAN_LANE_NAMESPACE: u64 = 0x636f_765f_666c_706d;

/// One independent patient's scalar outcome, group, and prespecified nuisance covariate.
#[derive(Clone, Debug)]
pub struct CovariatePatientRecord {
    /// Stable patient identifier.
    pub patient_id: String,
    /// Exact declared group label.
    pub group: String,
    /// Finite scalar outcome.
    pub outcome: f64,
    /// Finite prespecified nuisance covariate.
    pub covariate: f64,
}

/// Frozen one-covariate patient Freedman-Lane design.
#[derive(Clone, Debug)]
pub struct CovariatePermutationSpec {
    /// First group label; the target coefficient is group A minus group B.
    pub group_a: String,
    /// Second group label.
    pub group_b: String,
    /// Positive bounded residual-permutation count.
    pub permutations: usize,
    /// Base deterministic seed.
    pub seed: u64,
    /// Prespecified alternative for the adjusted group coefficient.
    pub alternative: PermutationAlternative,
}

/// Covariate-adjusted whole-patient residual-permutation result.
#[derive(Clone, Debug, PartialEq)]
pub struct CovariatePermutationResult {
    /// Exact covariate-conditional inference design.
    pub inference_design: InferenceDesign,
    /// Whether residual movement was restricted within exact patient blocks.
    pub blocked: bool,
    /// Number of exact blocks, or zero for the unblocked path.
    pub block_count: usize,
    /// Total independent patient count.
    pub patient_count: usize,
    /// Group A patient count.
    pub group_a_count: usize,
    /// Group B patient count.
    pub group_b_count: usize,
    /// Exact first group label.
    pub group_a: String,
    /// Exact second group label.
    pub group_b: String,
    /// Reduced-model column count: intercept plus one covariate.
    pub reduced_model_columns: usize,
    /// Full-model column count: intercept, covariate, and group indicator.
    pub full_model_columns: usize,
    /// Full-model residual degrees of freedom.
    pub residual_degrees_of_freedom: usize,
    /// Deterministic centering value applied to the nuisance covariate.
    pub covariate_center: f64,
    /// Positive deterministic max-absolute-deviation scale applied to the nuisance covariate.
    pub covariate_scale: f64,
    /// Adjusted group-A-minus-group-B coefficient.
    pub effect_group_a_minus_group_b: f64,
    /// Standard error of the adjusted group coefficient.
    pub target_standard_error: f64,
    /// Studentized adjusted group coefficient.
    pub studentized_statistic: f64,
    /// Inclusive-plus-one residual-permutation p-value.
    pub p_value: f64,
    /// Requested replicate count.
    pub permutations_requested: usize,
    /// Attempted replicate count.
    pub permutations_attempted: usize,
    /// Completed replicate count.
    pub permutations_completed: usize,
    /// Base seed.
    pub seed: u64,
    /// Prespecified alternative.
    pub alternative: PermutationAlternative,
}

/// Test an adjusted patient group coefficient by permuting reduced-model residuals.
pub fn patient_covariate_freedman_lane(
    records: &[CovariatePatientRecord],
    spec: &CovariatePermutationSpec,
) -> Result<CovariatePermutationResult, CohortInferenceError> {
    execute_covariate_freedman_lane(records, None, spec)
}

/// Restrict reduced-model residual movement within exact patient-ID-keyed blocks.
pub fn patient_blocked_covariate_freedman_lane(
    records: &[CovariatePatientRecord],
    assignments: &[PatientExchangeabilityBlock],
    spec: &CovariatePermutationSpec,
) -> Result<CovariatePermutationResult, CohortInferenceError> {
    execute_covariate_freedman_lane(records, Some(assignments), spec)
}

fn execute_covariate_freedman_lane(
    records: &[CovariatePatientRecord],
    assignments: Option<&[PatientExchangeabilityBlock]>,
    spec: &CovariatePermutationSpec,
) -> Result<CovariatePermutationResult, CohortInferenceError> {
    validate_spec(spec)?;
    let rows = canonicalize(records, spec)?;
    let work = rows.len().checked_mul(spec.permutations).ok_or_else(|| {
        CohortInferenceError::InvalidInput(
            "patient residual-permutation work exceeds the supported limit".into(),
        )
    })?;
    if work > MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "patient residual-permutation work exceeds the {MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS}-evaluation limit"
        )));
    }

    let outcomes = rows.iter().map(|row| row.outcome).collect::<Vec<_>>();
    let raw_covariates = rows.iter().map(|row| row.covariate).collect::<Vec<_>>();
    let covariate_center = stable_mean(&raw_covariates)?;
    let covariate_scale = raw_covariates
        .iter()
        .map(|value| (value - covariate_center).abs())
        .fold(0.0_f64, f64::max);
    if !covariate_scale.is_finite() || covariate_scale == 0.0 {
        return Err(CohortInferenceError::InvalidInput(
            "nuisance covariate must vary on a finite numerical scale".into(),
        ));
    }
    let covariates = raw_covariates
        .iter()
        .map(|value| (value - covariate_center) / covariate_scale)
        .collect::<Vec<_>>();
    let reduced = rows
        .iter()
        .zip(&covariates)
        .map(|(_, covariate)| vec![1.0, *covariate])
        .collect::<Vec<_>>();
    let full = rows
        .iter()
        .zip(&covariates)
        .map(|(row, covariate)| vec![1.0, *covariate, f64::from(row.group_a)])
        .collect::<Vec<_>>();
    let reduced_fit = fit_ols(&reduced, &outcomes, None)?;
    let observed = fit_ols(&full, &outcomes, Some(2))?;
    let residuals = outcomes
        .iter()
        .zip(&reduced_fit.fitted)
        .map(|(outcome, fitted)| outcome - fitted)
        .collect::<Vec<_>>();
    let design = match assignments {
        Some(assignments) => {
            let patient_ids = rows
                .iter()
                .map(|row| row.patient_id.clone())
                .collect::<Vec<_>>();
            let blocks = align_patient_blocks(&patient_ids, assignments, "covariate residual")?;
            InferenceDesign::blocked_covariate_conditional_residual_permutation(
                &blocks,
                spec.permutations,
                spec.seed,
                COVARIATE_FREEDMAN_LANE_NAMESPACE,
                inference_alternative(spec.alternative),
            )
        }
        None => InferenceDesign::covariate_conditional_residual_permutation(
            rows.len(),
            spec.permutations,
            spec.seed,
            COVARIATE_FREEDMAN_LANE_NAMESPACE,
            inference_alternative(spec.alternative),
        ),
    }
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;

    let mut lower_tail = 0usize;
    let mut upper_tail = 0usize;
    let mut permuted_outcomes = vec![0.0; rows.len()];
    for replicate in 0..spec.permutations {
        let indices = design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
        for index in 0..rows.len() {
            permuted_outcomes[index] = reduced_fit.fitted[index] + residuals[indices[index]];
        }
        let statistic = fit_ols(&full, &permuted_outcomes, Some(2))?.target_statistic;
        lower_tail += usize::from(statistic <= observed.target_statistic);
        upper_tail += usize::from(statistic >= observed.target_statistic);
    }
    let denominator = (spec.permutations + 1) as f64;
    let p_value = match spec.alternative {
        PermutationAlternative::Less => (lower_tail as f64 + 1.0) / denominator,
        PermutationAlternative::Greater => (upper_tail as f64 + 1.0) / denominator,
        PermutationAlternative::TwoSided => {
            (2.0 * ((lower_tail.min(upper_tail) as f64 + 1.0) / denominator)).min(1.0)
        }
    };
    let group_a_count = rows.iter().filter(|row| row.group_a).count();
    Ok(CovariatePermutationResult {
        blocked: assignments.is_some(),
        block_count: assignments.map_or(0, |_| design.block_count()),
        inference_design: design,
        patient_count: rows.len(),
        group_a_count,
        group_b_count: rows.len() - group_a_count,
        group_a: spec.group_a.clone(),
        group_b: spec.group_b.clone(),
        reduced_model_columns: reduced[0].len(),
        full_model_columns: full[0].len(),
        residual_degrees_of_freedom: observed.residual_degrees_of_freedom,
        covariate_center,
        covariate_scale,
        effect_group_a_minus_group_b: observed.target_coefficient,
        target_standard_error: observed.target_standard_error,
        studentized_statistic: observed.target_statistic,
        p_value,
        permutations_requested: spec.permutations,
        permutations_attempted: spec.permutations,
        permutations_completed: spec.permutations,
        seed: spec.seed,
        alternative: spec.alternative,
    })
}

fn validate_spec(spec: &CovariatePermutationSpec) -> Result<(), CohortInferenceError> {
    if spec.group_a.trim().is_empty() || spec.group_b.trim().is_empty() {
        return Err(CohortInferenceError::InvalidInput(
            "group labels must be non-empty".into(),
        ));
    }
    if spec.group_a.trim() != spec.group_a || spec.group_b.trim() != spec.group_b {
        return Err(CohortInferenceError::InvalidInput(
            "group labels may not have surrounding whitespace".into(),
        ));
    }
    if spec.group_a == spec.group_b {
        return Err(CohortInferenceError::InvalidInput(
            "group labels must be distinct".into(),
        ));
    }
    if spec.permutations == 0 || spec.permutations > MAXIMUM_PERMUTATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "permutations must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )));
    }
    Ok(())
}

#[derive(Clone)]
struct Row {
    patient_id: String,
    outcome: f64,
    covariate: f64,
    group_a: bool,
}

fn canonicalize(
    records: &[CovariatePatientRecord],
    spec: &CovariatePermutationSpec,
) -> Result<Vec<Row>, CohortInferenceError> {
    if records.len() < 6 || records.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "covariate-adjusted inference requires 6 to {MAXIMUM_PATIENTS} patients"
        )));
    }
    let mut rows = BTreeMap::<&str, Row>::new();
    for record in records {
        if record.patient_id.trim().is_empty()
            || record.patient_id.trim() != record.patient_id
            || !record.outcome.is_finite()
            || !record.covariate.is_finite()
        {
            return Err(CohortInferenceError::InvalidInput(
                "covariate rows require exact non-empty patient IDs and finite values".into(),
            ));
        }
        let group_a = if record.group == spec.group_a {
            true
        } else if record.group == spec.group_b {
            false
        } else {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has undeclared group {:?}",
                record.patient_id, record.group
            )));
        };
        if rows
            .insert(
                &record.patient_id,
                Row {
                    patient_id: record.patient_id.clone(),
                    outcome: record.outcome,
                    covariate: record.covariate,
                    group_a,
                },
            )
            .is_some()
        {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate patient row: {}",
                record.patient_id
            )));
        }
    }
    let rows = rows.into_values().collect::<Vec<_>>();
    let group_a_count = rows.iter().filter(|row| row.group_a).count();
    if group_a_count < 2 || rows.len() - group_a_count < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "each group must contain at least two patients".into(),
        ));
    }
    Ok(rows)
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
    target_index: Option<usize>,
) -> Result<OlsFit, CohortInferenceError> {
    let row_count = design.len();
    let column_count = design[0].len();
    if row_count <= column_count {
        return Err(CohortInferenceError::InvalidInput(
            "covariate model has no residual degrees of freedom".into(),
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
    let Some(target_index) = target_index else {
        return Ok(OlsFit {
            fitted,
            target_coefficient: 0.0,
            target_standard_error: 0.0,
            target_statistic: 0.0,
            residual_degrees_of_freedom,
        });
    };
    let rss = outcome
        .iter()
        .zip(&fitted)
        .map(|(actual, predicted)| (actual - predicted).powi(2))
        .sum::<f64>();
    let variance = rss / residual_degrees_of_freedom as f64;
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
            "covariate-adjusted target statistic is undefined or non-finite".into(),
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
    let scale = matrix
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max)
        .max(1.0);
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
        if augmented[selected][pivot].abs() <= 1e-12 * scale {
            return Err(CohortInferenceError::InvalidInput(
                "covariate model design is rank deficient".into(),
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
            "covariate model solve produced a non-finite coefficient".into(),
        ));
    }
    Ok(solution)
}

fn inference_alternative(alternative: PermutationAlternative) -> InferenceAlternative {
    match alternative {
        PermutationAlternative::Less => InferenceAlternative::Less,
        PermutationAlternative::Greater => InferenceAlternative::Greater,
        PermutationAlternative::TwoSided => InferenceAlternative::TwoSided,
    }
}

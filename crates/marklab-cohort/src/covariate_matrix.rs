use std::collections::{BTreeMap, HashSet};

use super::{
    covariate::{fit_ols, inference_alternative, validate_spec},
    inference_design::align_patient_blocks,
    numeric::stable_mean,
    CohortInferenceError, CovariatePermutationSpec, InferenceDesign, PatientExchangeabilityBlock,
    PermutationAlternative, MAXIMUM_PATIENTS,
};

const COVARIATE_MATRIX_NAMESPACE: u64 = 0x636f_765f_6d61_7478;
const MAXIMUM_NUISANCE_COVARIATES: usize = 32;
const MAXIMUM_COVARIATE_MATRIX_WORK: usize = 100_000_000;

/// One independent patient with an exact ordered nuisance-covariate vector.
#[derive(Clone, Debug)]
pub struct CovariateMatrixPatientRecord {
    /// Stable patient identifier.
    pub patient_id: String,
    /// Exact declared group label.
    pub group: String,
    /// Finite scalar outcome.
    pub outcome: f64,
    /// Exact ordered nuisance-column names shared by every patient.
    pub covariate_names: Vec<String>,
    /// Finite nuisance values aligned to `covariate_names`.
    pub covariates: Vec<f64>,
}

/// Multiple-covariate whole-patient residual-permutation result.
#[derive(Clone, Debug, PartialEq)]
pub struct CovariateMatrixPermutationResult {
    /// Exact residual-permutation design.
    pub inference_design: InferenceDesign,
    /// Whether residuals were restricted within exact patient blocks.
    pub blocked: bool,
    /// Exact block count, or zero for the unblocked path.
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
    /// Exact ordered nuisance-column names.
    pub covariate_names: Vec<String>,
    /// Deterministic column centers.
    pub covariate_centers: Vec<f64>,
    /// Positive deterministic max-absolute-deviation column scales.
    pub covariate_scales: Vec<f64>,
    /// Reduced-model column count: intercept plus nuisance columns.
    pub reduced_model_columns: usize,
    /// Full-model column count: reduced columns plus group indicator.
    pub full_model_columns: usize,
    /// Full-model residual degrees of freedom.
    pub residual_degrees_of_freedom: usize,
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

/// Adjust a patient group contrast for an exact fixed nuisance-covariate matrix.
pub fn patient_covariate_matrix_freedman_lane(
    records: &[CovariateMatrixPatientRecord],
    spec: &CovariatePermutationSpec,
) -> Result<CovariateMatrixPermutationResult, CohortInferenceError> {
    execute(records, None, spec)
}

/// Restrict nuisance-matrix residual movement within exact patient-ID-keyed blocks.
pub fn patient_blocked_covariate_matrix_freedman_lane(
    records: &[CovariateMatrixPatientRecord],
    assignments: &[PatientExchangeabilityBlock],
    spec: &CovariatePermutationSpec,
) -> Result<CovariateMatrixPermutationResult, CohortInferenceError> {
    execute(records, Some(assignments), spec)
}

fn execute(
    records: &[CovariateMatrixPatientRecord],
    assignments: Option<&[PatientExchangeabilityBlock]>,
    spec: &CovariatePermutationSpec,
) -> Result<CovariateMatrixPermutationResult, CohortInferenceError> {
    validate_spec(spec)?;
    let rows = canonicalize(records, spec)?;
    let nuisance_count = rows[0].covariates.len();
    let full_columns = nuisance_count + 2;
    let per_fit = rows
        .len()
        .checked_mul(full_columns)
        .and_then(|value| value.checked_mul(full_columns))
        .and_then(|value| value.checked_add(full_columns.pow(3)))
        .ok_or_else(work_limit_error)?;
    let work = per_fit
        .checked_mul(spec.permutations + 2)
        .ok_or_else(work_limit_error)?;
    if work > MAXIMUM_COVARIATE_MATRIX_WORK {
        return Err(work_limit_error());
    }

    let outcomes = rows.iter().map(|row| row.outcome).collect::<Vec<_>>();
    let mut centers = Vec::with_capacity(nuisance_count);
    let mut scales = Vec::with_capacity(nuisance_count);
    for column in 0..nuisance_count {
        let values = rows
            .iter()
            .map(|row| row.covariates[column])
            .collect::<Vec<_>>();
        let center = stable_mean(&values)?;
        let scale = values
            .iter()
            .map(|value| (value - center).abs())
            .fold(0.0_f64, f64::max);
        if !scale.is_finite() || scale == 0.0 {
            return Err(CohortInferenceError::InvalidInput(format!(
                "nuisance covariate {:?} must vary on a finite numerical scale",
                rows[0].covariate_names[column]
            )));
        }
        centers.push(center);
        scales.push(scale);
    }
    let reduced = rows
        .iter()
        .map(|row| {
            let mut design = Vec::with_capacity(nuisance_count + 1);
            design.push(1.0);
            design.extend(
                row.covariates
                    .iter()
                    .enumerate()
                    .map(|(column, value)| (value - centers[column]) / scales[column]),
            );
            design
        })
        .collect::<Vec<_>>();
    let full = reduced
        .iter()
        .zip(&rows)
        .map(|(reduced, row)| {
            let mut design = reduced.clone();
            design.push(f64::from(row.group_a));
            design
        })
        .collect::<Vec<_>>();
    let reduced_fit = fit_ols(&reduced, &outcomes, None)?;
    let target_index = full[0].len() - 1;
    let observed = fit_ols(&full, &outcomes, Some(target_index))?;
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
            let blocks = align_patient_blocks(&patient_ids, assignments, "covariate matrix")?;
            InferenceDesign::blocked_covariate_conditional_residual_permutation(
                &blocks,
                spec.permutations,
                spec.seed,
                COVARIATE_MATRIX_NAMESPACE,
                inference_alternative(spec.alternative),
            )
        }
        None => InferenceDesign::covariate_conditional_residual_permutation(
            rows.len(),
            spec.permutations,
            spec.seed,
            COVARIATE_MATRIX_NAMESPACE,
            inference_alternative(spec.alternative),
        ),
    }
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;

    let mut lower_tail = 0usize;
    let mut upper_tail = 0usize;
    let mut permuted = vec![0.0; rows.len()];
    for replicate in 0..spec.permutations {
        let indices = design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
        for index in 0..rows.len() {
            permuted[index] = reduced_fit.fitted[index] + residuals[indices[index]];
        }
        let statistic = fit_ols(&full, &permuted, Some(target_index))?.target_statistic;
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
    Ok(CovariateMatrixPermutationResult {
        blocked: assignments.is_some(),
        block_count: assignments.map_or(0, |_| design.block_count()),
        inference_design: design,
        patient_count: rows.len(),
        group_a_count,
        group_b_count: rows.len() - group_a_count,
        group_a: spec.group_a.clone(),
        group_b: spec.group_b.clone(),
        covariate_names: rows[0].covariate_names.clone(),
        covariate_centers: centers,
        covariate_scales: scales,
        reduced_model_columns: reduced[0].len(),
        full_model_columns: full[0].len(),
        residual_degrees_of_freedom: observed.residual_degrees_of_freedom,
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

#[derive(Clone)]
struct Row {
    patient_id: String,
    outcome: f64,
    group_a: bool,
    covariate_names: Vec<String>,
    covariates: Vec<f64>,
}

fn canonicalize(
    records: &[CovariateMatrixPatientRecord],
    spec: &CovariatePermutationSpec,
) -> Result<Vec<Row>, CohortInferenceError> {
    if records.len() < 6 || records.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "covariate-matrix inference requires 6 to {MAXIMUM_PATIENTS} patients"
        )));
    }
    let names = &records[0].covariate_names;
    if names.is_empty() || names.len() > MAXIMUM_NUISANCE_COVARIATES {
        return Err(CohortInferenceError::InvalidInput(format!(
            "covariate matrix requires 1 to {MAXIMUM_NUISANCE_COVARIATES} nuisance columns"
        )));
    }
    if names.iter().collect::<HashSet<_>>().len() != names.len()
        || names.iter().any(|name| {
            name.is_empty()
                || name.len() > 128
                || name.trim() != name
                || name.chars().any(char::is_control)
        })
    {
        return Err(CohortInferenceError::InvalidInput(
            "covariate names must be exact, unique, bounded, and non-empty".into(),
        ));
    }
    let mut rows = BTreeMap::<&str, Row>::new();
    for record in records {
        if record.patient_id.is_empty()
            || record.patient_id.trim() != record.patient_id
            || !record.outcome.is_finite()
            || record.covariate_names != *names
            || record.covariates.len() != names.len()
            || record.covariates.iter().any(|value| !value.is_finite())
        {
            return Err(CohortInferenceError::InvalidInput(
                "every patient requires one exact finite complete nuisance vector".into(),
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
                    group_a,
                    covariate_names: record.covariate_names.clone(),
                    covariates: record.covariates.clone(),
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

fn work_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "covariate-matrix OLS permutation work exceeds the {MAXIMUM_COVARIATE_MATRIX_WORK}-unit limit"
    ))
}

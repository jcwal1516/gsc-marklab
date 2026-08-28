use std::collections::{BTreeMap, HashSet};

use super::{
    covariate::{fit_ols, inference_alternative, validate_spec},
    numeric::stable_mean,
    CohortInferenceError, CovariatePermutationSpec, InferenceDesign, PermutationAlternative,
    MAXIMUM_PATIENTS,
};

const CLUSTER_COVARIATE_NAMESPACE: u64 = 0x636c_7573_636f_765f;
const MAXIMUM_NUISANCE_COVARIATES: usize = 32;
const MAXIMUM_CLUSTER_COVARIATE_WORK: usize = 100_000_000;

/// One patient nested in an exact cluster with a complete nuisance-covariate vector.
#[derive(Clone, Debug)]
pub struct ClusterCovariatePatientRecord {
    /// Globally unique patient identifier.
    pub patient_id: String,
    /// Exact cluster identifier.
    pub cluster_id: String,
    /// Exact declared cluster-level group label.
    pub group: String,
    /// Finite scalar patient outcome.
    pub outcome: f64,
    /// Exact ordered nuisance-column names shared by every patient.
    pub covariate_names: Vec<String>,
    /// Finite patient nuisance values aligned to `covariate_names`.
    pub covariates: Vec<f64>,
}

/// Covariate-adjusted residual permutation over equal-weight cluster summaries.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterCovariatePermutationResult {
    /// Exact cluster-level residual-permutation design.
    pub inference_design: InferenceDesign,
    /// Total nested patient count.
    pub patient_count: usize,
    /// Total independent cluster count.
    pub cluster_count: usize,
    /// Group A cluster count.
    pub group_a_cluster_count: usize,
    /// Group B cluster count.
    pub group_b_cluster_count: usize,
    /// Exact first group label.
    pub group_a: String,
    /// Exact second group label.
    pub group_b: String,
    /// Exact ordered nuisance-column names.
    pub covariate_names: Vec<String>,
    /// Deterministic cluster-summary column centers.
    pub covariate_centers: Vec<f64>,
    /// Positive deterministic max-absolute-deviation column scales.
    pub covariate_scales: Vec<f64>,
    /// Reduced-model column count: intercept plus nuisance columns.
    pub reduced_model_columns: usize,
    /// Full-model column count: reduced columns plus group indicator.
    pub full_model_columns: usize,
    /// Full-model cluster-level residual degrees of freedom.
    pub residual_degrees_of_freedom: usize,
    /// Adjusted cluster-level group-A-minus-group-B coefficient.
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
    /// Base deterministic seed.
    pub seed: u64,
    /// Prespecified alternative.
    pub alternative: PermutationAlternative,
}

/// Adjust an equal-weight cluster contrast for an exact fixed nuisance-covariate matrix.
pub fn cluster_covariate_matrix_freedman_lane(
    records: &[ClusterCovariatePatientRecord],
    spec: &CovariatePermutationSpec,
) -> Result<ClusterCovariatePermutationResult, CohortInferenceError> {
    validate_spec(spec)?;
    let clusters = canonical_cluster_summaries(records, spec)?;
    let nuisance_count = clusters[0].covariates.len();
    let full_columns = nuisance_count + 2;
    let per_fit = clusters
        .len()
        .checked_mul(full_columns)
        .and_then(|value| value.checked_mul(full_columns))
        .and_then(|value| value.checked_add(full_columns.pow(3)))
        .ok_or_else(work_limit_error)?;
    let work = per_fit
        .checked_mul(spec.permutations + 2)
        .ok_or_else(work_limit_error)?;
    if work > MAXIMUM_CLUSTER_COVARIATE_WORK {
        return Err(work_limit_error());
    }

    let outcomes = clusters.iter().map(|row| row.outcome).collect::<Vec<_>>();
    let mut centers = Vec::with_capacity(nuisance_count);
    let mut scales = Vec::with_capacity(nuisance_count);
    for column in 0..nuisance_count {
        let values = clusters
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
                "cluster nuisance covariate {:?} must vary on a finite numerical scale",
                clusters[0].covariate_names[column]
            )));
        }
        centers.push(center);
        scales.push(scale);
    }
    let reduced = clusters
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
        .zip(&clusters)
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
    let design = InferenceDesign::cluster_covariate_residual_permutation(
        clusters.len(),
        spec.permutations,
        spec.seed,
        CLUSTER_COVARIATE_NAMESPACE,
        inference_alternative(spec.alternative),
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;

    let mut lower_tail = 0usize;
    let mut upper_tail = 0usize;
    let mut permuted = vec![0.0; clusters.len()];
    for replicate in 0..spec.permutations {
        let indices = design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
        for index in 0..clusters.len() {
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
    let group_a_cluster_count = clusters.iter().filter(|row| row.group_a).count();
    Ok(ClusterCovariatePermutationResult {
        inference_design: design,
        patient_count: records.len(),
        cluster_count: clusters.len(),
        group_a_cluster_count,
        group_b_cluster_count: clusters.len() - group_a_cluster_count,
        group_a: spec.group_a.clone(),
        group_b: spec.group_b.clone(),
        covariate_names: clusters[0].covariate_names.clone(),
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
struct PatientRow {
    outcome: f64,
    covariates: Vec<f64>,
}

struct ClusterRows {
    group_a: bool,
    covariate_names: Vec<String>,
    patients: BTreeMap<String, PatientRow>,
}

struct ClusterSummary {
    group_a: bool,
    outcome: f64,
    covariate_names: Vec<String>,
    covariates: Vec<f64>,
}

fn canonical_cluster_summaries(
    records: &[ClusterCovariatePatientRecord],
    spec: &CovariatePermutationSpec,
) -> Result<Vec<ClusterSummary>, CohortInferenceError> {
    if records.is_empty() || records.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "cluster-covariate inference requires 1 to {MAXIMUM_PATIENTS} patient rows"
        )));
    }
    let names = &records[0].covariate_names;
    if names.is_empty() || names.len() > MAXIMUM_NUISANCE_COVARIATES {
        return Err(CohortInferenceError::InvalidInput(format!(
            "cluster covariate matrix requires 1 to {MAXIMUM_NUISANCE_COVARIATES} nuisance columns"
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
            "cluster covariate names must be exact, unique, bounded, and non-empty".into(),
        ));
    }
    let mut patient_ids = HashSet::with_capacity(records.len());
    let mut clusters = BTreeMap::<String, ClusterRows>::new();
    for record in records {
        if record.patient_id.is_empty()
            || record.patient_id.trim() != record.patient_id
            || record.cluster_id.is_empty()
            || record.cluster_id.trim() != record.cluster_id
            || !record.outcome.is_finite()
            || record.covariate_names != *names
            || record.covariates.len() != names.len()
            || record.covariates.iter().any(|value| !value.is_finite())
        {
            return Err(CohortInferenceError::InvalidInput(
                "every cluster patient requires exact IDs and one complete finite nuisance vector"
                    .into(),
            ));
        }
        if !patient_ids.insert(record.patient_id.as_str()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate cluster patient_id: {}",
                record.patient_id
            )));
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
        let cluster = clusters
            .entry(record.cluster_id.clone())
            .or_insert_with(|| ClusterRows {
                group_a,
                covariate_names: record.covariate_names.clone(),
                patients: BTreeMap::new(),
            });
        if cluster.group_a != group_a {
            return Err(CohortInferenceError::InvalidInput(format!(
                "cluster {} contains conflicting group labels",
                record.cluster_id
            )));
        }
        cluster.patients.insert(
            record.patient_id.clone(),
            PatientRow {
                outcome: record.outcome,
                covariates: record.covariates.clone(),
            },
        );
    }
    let mut summaries = Vec::with_capacity(clusters.len());
    for cluster in clusters.into_values() {
        let outcomes = cluster
            .patients
            .values()
            .map(|row| row.outcome)
            .collect::<Vec<_>>();
        let mut covariates = Vec::with_capacity(names.len());
        for column in 0..names.len() {
            let values = cluster
                .patients
                .values()
                .map(|row| row.covariates[column])
                .collect::<Vec<_>>();
            covariates.push(stable_mean(&values)?);
        }
        summaries.push(ClusterSummary {
            group_a: cluster.group_a,
            outcome: stable_mean(&outcomes)?,
            covariate_names: cluster.covariate_names,
            covariates,
        });
    }
    let group_a_count = summaries.iter().filter(|row| row.group_a).count();
    if group_a_count < 2 || summaries.len() - group_a_count < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "each group must contain at least two independent clusters".into(),
        ));
    }
    Ok(summaries)
}

fn work_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "cluster covariate OLS permutation work exceeds the {MAXIMUM_CLUSTER_COVARIATE_WORK}-unit limit"
    ))
}

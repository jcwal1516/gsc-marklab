use std::{collections::BTreeMap, fs, path::PathBuf};

use marklab_cohort::{
    cluster_covariate_matrix_freedman_lane, ClusterCovariatePatientRecord,
    ClusterCovariatePermutationResult, CovariatePermutationSpec, InferenceAnalysisLevel,
    InferenceNullFamily, InferencePermutationUnit,
};
use serde::{Deserialize, Serialize};

use super::{publication::publish_json, CliAlternative, CohortError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    patient_id: String,
    cluster_id: String,
    group: String,
    outcome: f64,
    covariate: String,
    value: f64,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    design: Design,
    patients: usize,
    clusters: ClusterCounts,
    groups: Groups,
    covariates: Covariates,
    effect_group_a_minus_group_b: f64,
    target_standard_error: f64,
    studentized_statistic: f64,
    p_value: f64,
    permutations: Permutations,
    claim_status: &'static str,
}

#[derive(Debug, Serialize)]
struct Design {
    analysis_level: &'static str,
    null_family: &'static str,
    permutation_unit: &'static str,
    cluster_summary: &'static str,
    nuisance_columns: usize,
    reduced_model_columns: usize,
    full_model_columns: usize,
    residual_degrees_of_freedom: usize,
    exchangeability_assumption: &'static str,
}

#[derive(Debug, Serialize)]
struct ClusterCounts {
    total: usize,
    group_a: usize,
    group_b: usize,
}

#[derive(Debug, Serialize)]
struct Groups {
    group_a: String,
    group_b: String,
}

#[derive(Debug, Serialize)]
struct Covariates {
    names: Vec<String>,
    centers: Vec<f64>,
    scales: Vec<f64>,
}

#[derive(Debug, Serialize)]
struct Permutations {
    requested: usize,
    attempted: usize,
    completed: usize,
    seed: u64,
    alternative: CliAlternative,
}

pub(super) struct RunArgs {
    pub input: PathBuf,
    pub group_a: String,
    pub group_b: String,
    pub permutations: usize,
    pub seed: u64,
    pub alternative: CliAlternative,
    pub out: PathBuf,
}

pub(super) fn run(args: RunArgs) -> Result<(), CohortError> {
    let records = read_records(&args.input)?;
    let result = cluster_covariate_matrix_freedman_lane(
        &records,
        &CovariatePermutationSpec {
            group_a: args.group_a,
            group_b: args.group_b,
            permutations: args.permutations,
            seed: args.seed,
            alternative: args.alternative.into(),
        },
    )?;
    publish_json(&args.out, &Output::from_result(result, args.alternative))
}

impl Output {
    fn from_result(result: ClusterCovariatePermutationResult, alternative: CliAlternative) -> Self {
        match result.inference_design.analysis_level() {
            InferenceAnalysisLevel::Cluster => {}
            _ => unreachable!("cluster covariate inference returned another analysis level"),
        }
        match result.inference_design.null_family() {
            InferenceNullFamily::ClusterCovariateResidualPermutation => {}
            _ => unreachable!("cluster covariate inference returned another null family"),
        }
        match result.inference_design.permutation_unit() {
            InferencePermutationUnit::CompleteClusterResidual => {}
            _ => unreachable!("cluster covariate inference returned another permutation unit"),
        }
        Self {
            format: "marklab.cohort_cluster_covariate_permutation",
            version: 1,
            design: Design {
                analysis_level: "cluster",
                null_family: "cluster_covariate_residual_permutation",
                permutation_unit: "complete_cluster_residual",
                cluster_summary: "equal_weight_patient_mean",
                nuisance_columns: result.covariate_names.len(),
                reduced_model_columns: result.reduced_model_columns,
                full_model_columns: result.full_model_columns,
                residual_degrees_of_freedom: result.residual_degrees_of_freedom,
                exchangeability_assumption:
                    "reduced-model residuals are exchangeable across independent clusters conditional on the fixed cluster-level nuisance matrix",
            },
            patients: result.patient_count,
            clusters: ClusterCounts {
                total: result.cluster_count,
                group_a: result.group_a_cluster_count,
                group_b: result.group_b_cluster_count,
            },
            groups: Groups {
                group_a: result.group_a,
                group_b: result.group_b,
            },
            covariates: Covariates {
                names: result.covariate_names,
                centers: result.covariate_centers,
                scales: result.covariate_scales,
            },
            effect_group_a_minus_group_b: result.effect_group_a_minus_group_b,
            target_standard_error: result.target_standard_error,
            studentized_statistic: result.studentized_statistic,
            p_value: result.p_value,
            permutations: Permutations {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
                seed: result.seed,
                alternative,
            },
            claim_status: "experimental_cluster_residual_exchangeability_required",
        }
    }
}

fn read_records(path: &std::path::Path) -> Result<Vec<ClusterCovariatePatientRecord>, CohortError> {
    let metadata = fs::metadata(path).map_err(|source| CohortError::Output {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CohortError::Input(
            "cluster-covariate input must be a regular file within 16 MiB".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    if !reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .iter()
        .eq([
            "patient_id",
            "cluster_id",
            "group",
            "outcome",
            "covariate",
            "value",
        ])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,cluster_id,group,outcome,covariate,value".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, String, f64, BTreeMap<String, f64>)>::new();
    for row in reader.deserialize::<CsvRow>() {
        let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
        let entry = grouped.entry(row.patient_id.clone()).or_insert_with(|| {
            (
                row.cluster_id.clone(),
                row.group.clone(),
                row.outcome,
                BTreeMap::new(),
            )
        });
        if entry.0 != row.cluster_id
            || entry.1 != row.group
            || entry.2.to_bits() != row.outcome.to_bits()
        {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting cluster, group, or outcome values",
                row.patient_id
            )));
        }
        if entry.3.insert(row.covariate.clone(), row.value).is_some() {
            return Err(CohortError::Input(format!(
                "patient {} has duplicate covariate {:?}",
                row.patient_id, row.covariate
            )));
        }
    }
    Ok(grouped
        .into_iter()
        .map(|(patient_id, (cluster_id, group, outcome, covariates))| {
            ClusterCovariatePatientRecord {
                patient_id,
                cluster_id,
                group,
                outcome,
                covariates: covariates.values().copied().collect(),
                covariate_names: covariates.into_keys().collect(),
            }
        })
        .collect())
}

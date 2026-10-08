use std::{collections::BTreeMap, fs, path::PathBuf};

use marklab_cohort::{
    cluster_covariate_matrix_freedman_lane, ClusterCovariatePatientRecord,
    ClusterCovariatePermutationResult, CovariatePermutationSpec, InferenceAnalysisLevel,
    InferenceNullFamily, InferencePermutationUnit,
};
use serde::{Deserialize, Serialize};

use super::{
    input::validate_input_file_with_message, publication::publish_json, CliAlternative, CohortError,
};

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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    pub(crate) format: String,
    pub(crate) version: u32,
    pub(crate) design: Design,
    pub(crate) patients: usize,
    pub(crate) clusters: ClusterCounts,
    pub(crate) groups: Groups,
    pub(crate) covariates: Covariates,
    pub(crate) effect_group_a_minus_group_b: f64,
    pub(crate) target_standard_error: f64,
    pub(crate) studentized_statistic: f64,
    pub(crate) p_value: f64,
    pub(crate) permutations: Permutations,
    pub(crate) claim_status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Design {
    pub(crate) analysis_level: String,
    pub(crate) null_family: String,
    pub(crate) permutation_unit: String,
    pub(crate) cluster_summary: String,
    pub(crate) nuisance_columns: usize,
    pub(crate) reduced_model_columns: usize,
    pub(crate) full_model_columns: usize,
    pub(crate) residual_degrees_of_freedom: usize,
    pub(crate) exchangeability_assumption: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClusterCounts {
    pub(crate) total: usize,
    pub(crate) group_a: usize,
    pub(crate) group_b: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Groups {
    pub(crate) group_a: String,
    pub(crate) group_b: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Covariates {
    pub(crate) names: Vec<String>,
    pub(crate) centers: Vec<f64>,
    pub(crate) scales: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Permutations {
    pub(crate) requested: usize,
    pub(crate) attempted: usize,
    pub(crate) completed: usize,
    pub(crate) seed: u64,
    pub(crate) alternative: CliAlternative,
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
    pub(crate) fn from_result(
        result: ClusterCovariatePermutationResult,
        alternative: CliAlternative,
    ) -> Self {
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
            format: "marklab.cohort_cluster_covariate_permutation".into(),
            version: 1,
            design: Design {
                analysis_level: "cluster".into(),
                null_family: "cluster_covariate_residual_permutation".into(),
                permutation_unit: "complete_cluster_residual".into(),
                cluster_summary: "equal_weight_patient_mean".into(),
                nuisance_columns: result.covariate_names.len(),
                reduced_model_columns: result.reduced_model_columns,
                full_model_columns: result.full_model_columns,
                residual_degrees_of_freedom: result.residual_degrees_of_freedom,
                exchangeability_assumption:
                    "reduced-model residuals are exchangeable across independent clusters conditional on the fixed cluster-level nuisance matrix".into(),
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
            claim_status: "experimental_cluster_residual_exchangeability_required".into(),
        }
    }
}

fn read_records(path: &std::path::Path) -> Result<Vec<ClusterCovariatePatientRecord>, CohortError> {
    validate_input_file_with_message(
        path,
        "cluster-covariate input must be a regular file within 16 MiB",
    )?;
    let bytes = fs::read(path).map_err(|error| CohortError::Input(error.to_string()))?;
    read_records_from_bytes(&bytes)
}

pub(crate) fn read_records_from_bytes(
    bytes: &[u8],
) -> Result<Vec<ClusterCovariatePatientRecord>, CohortError> {
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
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

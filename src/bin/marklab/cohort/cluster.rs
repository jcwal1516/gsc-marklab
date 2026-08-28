use std::path::PathBuf;

use marklab_cohort::{
    cluster_level_permutation_test, ClusterPatientEndpoint, ClusterPermutationResult,
    ClusterPermutationSpec, InferenceAnalysisLevel, InferenceNullFamily, InferencePermutationUnit,
};
use serde::{Deserialize, Serialize};

use super::{publication::publish_json, validate_input_file, CliAlternative, CohortError};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    patient_id: String,
    cluster_id: String,
    group: String,
    endpoint: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input: PathBuf,
    group_a: String,
    group_b: String,
    permutations: usize,
    seed: u64,
    alternative: CliAlternative,
    out: PathBuf,
) -> Result<(), CohortError> {
    let records = read_records(&input)?;
    let result = cluster_level_permutation_test(
        &records,
        &ClusterPermutationSpec {
            group_a,
            group_b,
            permutations,
            seed,
            alternative: alternative.into(),
        },
    )?;
    publish_json(&out, &Output::from_result(input, alternative, result))
}

fn read_records(path: &std::path::Path) -> Result<Vec<ClusterPatientEndpoint>, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    if !reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .iter()
        .eq(["patient_id", "cluster_id", "group", "endpoint"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,cluster_id,group,endpoint".into(),
        ));
    }
    reader
        .deserialize::<CsvRow>()
        .map(|row| {
            let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(ClusterPatientEndpoint {
                patient_id: row.patient_id,
                cluster_id: row.cluster_id,
                group: row.group,
                endpoint: row.endpoint,
            })
        })
        .collect()
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: Design,
    patients: usize,
    clusters: ClusterCounts,
    group_a_mean: f64,
    group_b_mean: f64,
    effect_group_a_minus_group_b: f64,
    studentized_statistic: f64,
    p_value: f64,
    permutations: Permutations,
    seed: u64,
    alternative: CliAlternative,
}

#[derive(Debug, Serialize)]
struct Design {
    analysis_level: &'static str,
    null_family: &'static str,
    permutation_unit: &'static str,
    cluster_endpoint: &'static str,
}

#[derive(Debug, Serialize)]
struct ClusterCounts {
    total: usize,
    group_a: usize,
    group_b: usize,
}

#[derive(Debug, Serialize)]
struct Permutations {
    requested: usize,
    attempted: usize,
    completed: usize,
}

impl Output {
    fn from_result(
        input: PathBuf,
        alternative: CliAlternative,
        result: ClusterPermutationResult,
    ) -> Self {
        let analysis_level = match result.inference_design.analysis_level() {
            InferenceAnalysisLevel::Cluster => "cluster",
            _ => unreachable!("cluster permutation returned another analysis level"),
        };
        let null_family = match result.inference_design.null_family() {
            InferenceNullFamily::ClusterLabelPermutation => "cluster_label_permutation",
            _ => unreachable!("cluster permutation returned another null family"),
        };
        let permutation_unit = match result.inference_design.permutation_unit() {
            InferencePermutationUnit::CompleteClusterEndpoint => "complete_cluster_endpoint",
            _ => unreachable!("cluster permutation returned another unit"),
        };
        Self {
            format: "marklab.cohort_cluster_permutation",
            version: 1,
            input,
            design: Design {
                analysis_level,
                null_family,
                permutation_unit,
                cluster_endpoint: "equal_weight_patient_mean",
            },
            patients: result.patient_count,
            clusters: ClusterCounts {
                total: result.cluster_count,
                group_a: result.group_a_cluster_count,
                group_b: result.group_b_cluster_count,
            },
            group_a_mean: result.group_a_mean,
            group_b_mean: result.group_b_mean,
            effect_group_a_minus_group_b: result.effect_group_a_minus_group_b,
            studentized_statistic: result.studentized_statistic,
            p_value: result.p_value,
            permutations: Permutations {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
            },
            seed: result.seed,
            alternative,
        }
    }
}

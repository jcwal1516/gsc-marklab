use std::{fs, path::PathBuf};

use marklab_cohort::{
    repeated_measures_freedman_lane, RepeatedFreedmanLaneResult, RepeatedFreedmanLaneSpec,
    RepeatedMeasureRecord,
};
use serde::{Deserialize, Serialize};

use super::{publication::publish_json, CohortError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    subject_id: String,
    visit_id: String,
    outcome: f64,
    target: f64,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    design: Design,
    target_coefficient: f64,
    target_standard_error: f64,
    studentized_statistic: f64,
    p_value: f64,
    permutations: Permutations,
    claim_status: &'static str,
}

#[derive(Debug, Serialize)]
struct Design {
    subject_count: usize,
    row_count: usize,
    full_model_columns: usize,
    reduced_model_columns: usize,
    residual_degrees_of_freedom: usize,
    target_columns: [&'static str; 1],
    residual_randomization: &'static str,
    exchangeability_assumption: &'static str,
}

#[derive(Debug, Serialize)]
struct Permutations {
    requested: usize,
    completed: usize,
    seed: u64,
    alternative: &'static str,
}

pub(super) fn run(
    input: PathBuf,
    permutations: usize,
    seed: u64,
    out: PathBuf,
) -> Result<(), CohortError> {
    let records = read_records(&input)?;
    let result = repeated_measures_freedman_lane(
        &records,
        &RepeatedFreedmanLaneSpec { permutations, seed },
    )?;
    publish_json(&out, &Output::from(result))
}

impl From<RepeatedFreedmanLaneResult> for Output {
    fn from(result: RepeatedFreedmanLaneResult) -> Self {
        Self {
            format: "marklab.cohort_repeated_freedman_lane",
            version: 1,
            design: Design {
                subject_count: result.subject_count,
                row_count: result.row_count,
                full_model_columns: result.full_model_columns,
                reduced_model_columns: result.reduced_model_columns,
                residual_degrees_of_freedom: result.residual_degrees_of_freedom,
                target_columns: ["target"],
                residual_randomization: "whole_subject_sign_flip",
                exchangeability_assumption:
                    "reduced-model residual vectors are sign-exchangeable by independent subject",
            },
            target_coefficient: result.target_coefficient,
            target_standard_error: result.target_standard_error,
            studentized_statistic: result.studentized_statistic,
            p_value: result.p_value,
            permutations: Permutations {
                requested: result.permutations_requested,
                completed: result.permutations_completed,
                seed: result.seed,
                alternative: "two_sided",
            },
            claim_status: "experimental_residual_exchangeability_required",
        }
    }
}

fn read_records(path: &std::path::Path) -> Result<Vec<RepeatedMeasureRecord>, CohortError> {
    let metadata = fs::metadata(path).map_err(|source| CohortError::Output {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CohortError::Input(
            "repeated input must be a regular file within 16 MiB".into(),
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
        .eq(["subject_id", "visit_id", "outcome", "target"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly subject_id,visit_id,outcome,target".into(),
        ));
    }
    reader
        .deserialize::<CsvRow>()
        .map(|row| {
            let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(RepeatedMeasureRecord {
                subject_id: row.subject_id,
                visit_id: row.visit_id,
                outcome: row.outcome,
                target: row.target,
            })
        })
        .collect()
}

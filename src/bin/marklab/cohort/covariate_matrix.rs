use std::{collections::BTreeMap, path::PathBuf};

use marklab_cohort::{
    patient_blocked_covariate_matrix_freedman_lane, patient_covariate_matrix_freedman_lane,
    CovariateMatrixPatientRecord, CovariateMatrixPermutationResult, CovariatePermutationSpec,
    InferenceNullFamily, InferencePermutationUnit, PatientExchangeabilityBlock,
};
use serde::{Deserialize, Serialize};

use super::{
    input::validate_input_file_with_message, publication::publish_json, CliAlternative, CohortError,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    patient_id: String,
    group: String,
    outcome: f64,
    covariate: String,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

struct Input {
    records: Vec<CovariateMatrixPatientRecord>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    design: Design,
    patients: Patients,
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
    randomization_unit: &'static str,
    null_family: &'static str,
    permutation_unit: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    block_count: Option<usize>,
    nuisance_columns: usize,
    reduced_model_columns: usize,
    full_model_columns: usize,
    residual_degrees_of_freedom: usize,
    exchangeability_assumption: &'static str,
}

#[derive(Debug, Serialize)]
struct Patients {
    total: usize,
}

#[derive(Debug, Serialize)]
struct Groups {
    group_a: String,
    group_b: String,
    group_a_patients: usize,
    group_b_patients: usize,
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
    let input = read_records(&args.input)?;
    let spec = CovariatePermutationSpec {
        group_a: args.group_a,
        group_b: args.group_b,
        permutations: args.permutations,
        seed: args.seed,
        alternative: args.alternative.into(),
    };
    let result = match input.blocks {
        Some(blocks) => {
            patient_blocked_covariate_matrix_freedman_lane(&input.records, &blocks, &spec)?
        }
        None => patient_covariate_matrix_freedman_lane(&input.records, &spec)?,
    };
    publish_json(&args.out, &Output::from_result(result, args.alternative))
}

impl Output {
    fn from_result(result: CovariateMatrixPermutationResult, alternative: CliAlternative) -> Self {
        match result.inference_design.null_family() {
            InferenceNullFamily::CovariateConditionalResidualPermutation => {}
            _ => unreachable!("covariate matrix returned another null family"),
        }
        match result.inference_design.permutation_unit() {
            InferencePermutationUnit::CompletePatientResidual => {}
            _ => unreachable!("covariate matrix returned another permutation unit"),
        }
        Self {
            format: "marklab.cohort_covariate_matrix_permutation",
            version: 1,
            design: Design {
                randomization_unit: "patient_residual",
                null_family: "covariate_conditional_residual_permutation",
                permutation_unit: "complete_patient_residual",
                blocked: result.blocked.then_some(true),
                block_count: result.blocked.then_some(result.block_count),
                nuisance_columns: result.covariate_names.len(),
                reduced_model_columns: result.reduced_model_columns,
                full_model_columns: result.full_model_columns,
                residual_degrees_of_freedom: result.residual_degrees_of_freedom,
                exchangeability_assumption:
                    "reduced-model residuals are exchangeable across independent patients within declared blocks conditional on the fixed nuisance matrix",
            },
            patients: Patients {
                total: result.patient_count,
            },
            groups: Groups {
                group_a: result.group_a,
                group_b: result.group_b,
                group_a_patients: result.group_a_count,
                group_b_patients: result.group_b_count,
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
            claim_status: "experimental_residual_exchangeability_required",
        }
    }
}

fn read_records(path: &std::path::Path) -> Result<Input, CohortError> {
    validate_input_file_with_message(
        path,
        "covariate-matrix input must be a regular file within 16 MiB",
    )?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let blocked = headers.iter().eq([
        "patient_id",
        "group",
        "outcome",
        "covariate",
        "value",
        "block",
    ]);
    if !blocked
        && !headers
            .iter()
            .eq(["patient_id", "group", "outcome", "covariate", "value"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,outcome,covariate,value with optional trailing block".into(),
        ));
    }
    let mut grouped =
        BTreeMap::<String, (String, f64, Option<String>, BTreeMap<String, f64>)>::new();
    for row in reader.deserialize::<CsvRow>() {
        let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
        let entry = grouped.entry(row.patient_id.clone()).or_insert_with(|| {
            (
                row.group.clone(),
                row.outcome,
                row.block.clone(),
                BTreeMap::new(),
            )
        });
        if entry.0 != row.group
            || entry.1.to_bits() != row.outcome.to_bits()
            || entry.2 != row.block
        {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting group, outcome, or block values",
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
    let mut records = Vec::with_capacity(grouped.len());
    let mut blocks = blocked.then(|| Vec::with_capacity(grouped.len()));
    for (patient_id, (group, outcome, block, covariates)) in grouped {
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(patient_id.clone(), block.unwrap_or_default())
                    .map_err(|error| CohortError::Input(error.to_string()))?,
            );
        }
        records.push(CovariateMatrixPatientRecord {
            patient_id,
            group,
            outcome,
            covariates: covariates.values().copied().collect(),
            covariate_names: covariates.into_keys().collect(),
        });
    }
    Ok(Input { records, blocks })
}

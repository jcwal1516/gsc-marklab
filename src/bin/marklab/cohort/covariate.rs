use std::path::PathBuf;

use marklab_cohort::{
    patient_blocked_covariate_freedman_lane, patient_covariate_freedman_lane,
    CovariatePatientRecord, CovariatePermutationResult, CovariatePermutationSpec,
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
    covariate: f64,
    #[serde(default)]
    block: Option<String>,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    design: Design,
    patients: Patients,
    groups: Groups,
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
    reduced_model_columns: usize,
    full_model_columns: usize,
    residual_degrees_of_freedom: usize,
    covariate_center: f64,
    covariate_scale: f64,
    nuisance_columns: [&'static str; 1],
    target_columns: [&'static str; 1],
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
        Some(blocks) => patient_blocked_covariate_freedman_lane(&input.records, &blocks, &spec)?,
        None => patient_covariate_freedman_lane(&input.records, &spec)?,
    };
    publish_json(&args.out, &Output::from_result(result, args.alternative))
}

impl Output {
    fn from_result(result: CovariatePermutationResult, alternative: CliAlternative) -> Self {
        match result.inference_design.null_family() {
            InferenceNullFamily::CovariateConditionalResidualPermutation => {}
            _ => unreachable!("covariate permutation returned another null family"),
        }
        match result.inference_design.permutation_unit() {
            InferencePermutationUnit::CompletePatientResidual => {}
            _ => unreachable!("covariate permutation returned another permutation unit"),
        }
        Self {
            format: "marklab.cohort_covariate_permutation",
            version: 1,
            design: Design {
                randomization_unit: "patient_residual",
                null_family: "covariate_conditional_residual_permutation",
                permutation_unit: "complete_patient_residual",
                blocked: result.blocked.then_some(true),
                block_count: result.blocked.then_some(result.block_count),
                reduced_model_columns: result.reduced_model_columns,
                full_model_columns: result.full_model_columns,
                residual_degrees_of_freedom: result.residual_degrees_of_freedom,
                covariate_center: result.covariate_center,
                covariate_scale: result.covariate_scale,
                nuisance_columns: ["covariate"],
                target_columns: ["group_a_indicator"],
                exchangeability_assumption:
                    "reduced-model residuals are exchangeable across independent patients conditional on the fixed covariate",
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

struct Input {
    records: Vec<CovariatePatientRecord>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

fn read_records(path: &std::path::Path) -> Result<Input, CohortError> {
    validate_input_file_with_message(path, "covariate input must be a regular file within 16 MiB")?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "outcome", "covariate", "block"]);
    if !blocked
        && !headers
            .iter()
            .eq(["patient_id", "group", "outcome", "covariate"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,outcome,covariate with optional trailing block".into(),
        ));
    }
    let mut records = Vec::new();
    let mut blocks = blocked.then(Vec::new);
    for row in reader.deserialize::<CsvRow>() {
        let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(
                    row.patient_id.clone(),
                    row.block.clone().unwrap_or_default(),
                )
                .map_err(|error| CohortError::Input(error.to_string()))?,
            );
        }
        records.push(CovariatePatientRecord {
            patient_id: row.patient_id,
            group: row.group,
            outcome: row.outcome,
            covariate: row.covariate,
        });
    }
    Ok(Input { records, blocks })
}

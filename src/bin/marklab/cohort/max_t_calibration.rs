use std::{fs, path::PathBuf};

use marklab_cohort::{
    exact_max_t_calibration, MaxTCalibrationMethodResult, MaxTCalibrationRecord,
    MaxTCalibrationResult, MaxTCalibrationSpec,
};
use serde::{Deserialize, Serialize};

use super::{publication::publish_json, validate_input_file, CohortError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MaxTCalibrationConfiguration {
    pub group_a_count: usize,
    pub family_sizes: Vec<usize>,
    pub alpha: f64,
    pub maximum_assignments: usize,
    pub maximum_assignment_endpoint_evaluations: u64,
    pub memory_budget_bytes: usize,
}

pub(crate) struct PreparedMaxTCalibration {
    pub input: PathBuf,
    pub input_bytes: Vec<u8>,
    pub records: Vec<MaxTCalibrationRecord>,
    pub configuration: MaxTCalibrationConfiguration,
}

pub(crate) struct RunArgs {
    pub input: PathBuf,
    pub group_a_count: usize,
    pub family_sizes: Vec<usize>,
    pub alpha: f64,
    pub maximum_assignments: usize,
    pub maximum_assignment_endpoint_evaluations: u64,
    pub memory_budget_mib: usize,
    pub out: PathBuf,
}

pub(crate) fn run(args: RunArgs) -> Result<(), CohortError> {
    let prepared = prepare(
        args.input,
        args.group_a_count,
        args.family_sizes,
        args.alpha,
        args.maximum_assignments,
        args.maximum_assignment_endpoint_evaluations,
        args.memory_budget_mib,
    )?;
    let output = execute(&prepared)?;
    publish_json(&args.out, &output)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    input: PathBuf,
    group_a_count: usize,
    family_sizes: Vec<usize>,
    alpha: f64,
    maximum_assignments: usize,
    maximum_assignment_endpoint_evaluations: u64,
    memory_budget_mib: usize,
) -> Result<PreparedMaxTCalibration, CohortError> {
    validate_input_file(&input)?;
    let metadata = fs::metadata(&input).map_err(|error| CohortError::Input(error.to_string()))?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CohortError::Input(format!(
            "Max-T calibration input exceeds {MAXIMUM_INPUT_BYTES} bytes"
        )));
    }
    let input_bytes = fs::read(&input).map_err(|error| CohortError::Input(error.to_string()))?;
    let records = parse_records(&input_bytes)?;
    let memory_budget_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CohortError::Input("memory budget overflowed".into()))?;
    let configuration = MaxTCalibrationConfiguration {
        group_a_count,
        family_sizes,
        alpha,
        maximum_assignments,
        maximum_assignment_endpoint_evaluations,
        memory_budget_bytes,
    };
    Ok(PreparedMaxTCalibration {
        input,
        input_bytes,
        records,
        configuration,
    })
}

pub(crate) fn execute(
    prepared: &PreparedMaxTCalibration,
) -> Result<MaxTCalibrationOutput, CohortError> {
    let result = exact_max_t_calibration(
        &prepared.records,
        &MaxTCalibrationSpec {
            group_a_count: prepared.configuration.group_a_count,
            family_sizes: prepared.configuration.family_sizes.clone(),
            alpha: prepared.configuration.alpha,
            maximum_assignments: prepared.configuration.maximum_assignments,
            maximum_assignment_endpoint_evaluations: prepared
                .configuration
                .maximum_assignment_endpoint_evaluations,
            memory_budget_bytes: prepared.configuration.memory_budget_bytes,
        },
    )?;
    Ok(MaxTCalibrationOutput::from_result(
        prepared.input.clone(),
        result,
    ))
}

fn parse_records(bytes: &[u8]) -> Result<Vec<MaxTCalibrationRecord>, CohortError> {
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    if headers.len() < 2 || headers.get(0) != Some("patient_id") {
        return Err(CohortError::Input(
            "calibration CSV must begin patient_id followed by endpoint columns".into(),
        ));
    }
    let endpoints = headers
        .iter()
        .skip(1)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    reader
        .records()
        .map(|row| {
            let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
            let patient_id = row
                .get(0)
                .ok_or_else(|| CohortError::Input("calibration row lacks patient_id".into()))?
                .to_owned();
            let values = row
                .iter()
                .skip(1)
                .map(|value| {
                    value.parse::<f64>().map_err(|error| {
                        CohortError::Input(format!("invalid calibration endpoint value: {error}"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(MaxTCalibrationRecord {
                patient_id,
                endpoints: endpoints.clone(),
                values,
            })
        })
        .collect()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MaxTCalibrationOutput {
    format: String,
    version: u32,
    input: PathBuf,
    population_unit: String,
    null: String,
    assumptions: [String; 3],
    failure_policy: String,
    patient_count: usize,
    group_a_count: usize,
    group_b_count: usize,
    endpoints: Vec<String>,
    family_sizes: Vec<usize>,
    exact_assignment_count: usize,
    alpha: f64,
    single_step: MethodOutput,
    step_down: MethodOutput,
    ordered_gatekeeping_step_down: MethodOutput,
    work: WorkOutput,
}

impl MaxTCalibrationOutput {
    fn from_result(input: PathBuf, result: MaxTCalibrationResult) -> Self {
        Self {
            format: "marklab.cohort_max_t_calibration".into(),
            version: 1,
            input,
            population_unit: result.population_unit,
            null: result.null,
            assumptions: [
                "the_complete_patient_endpoint_table_is_fixed".into(),
                "every_fixed_size_group_assignment_is_equally_likely".into(),
                "ordered_endpoint_families_are_prespecified".into(),
            ],
            failure_policy: "reject_incomplete_nonfinite_degenerate_or_over_budget_exact_designs"
                .into(),
            patient_count: result.patient_count,
            group_a_count: result.group_a_count,
            group_b_count: result.group_b_count,
            endpoints: result.endpoints,
            family_sizes: result.family_sizes,
            exact_assignment_count: result.exact_assignment_count,
            alpha: result.alpha,
            single_step: result.single_step.into(),
            step_down: result.step_down.into(),
            ordered_gatekeeping_step_down: result.ordered_gatekeeping_step_down.into(),
            work: WorkOutput {
                assignments: result.work.assignments,
                statistic_patient_endpoint_evaluations: result
                    .work
                    .statistic_patient_endpoint_evaluations,
                assignment_endpoint_comparisons: result.work.assignment_endpoint_comparisons,
                retained_memory_bytes: result.work.retained_memory_bytes,
            },
        }
    }

    pub(crate) fn validates(&self, prepared: &PreparedMaxTCalibration) -> bool {
        let assignment_count =
            exact_assignment_count(prepared.records.len(), prepared.configuration.group_a_count);
        let expected_comparisons = assignment_count.and_then(|assignments| {
            comparison_work(
                assignments,
                prepared.records[0].endpoints.len(),
                &prepared.configuration.family_sizes,
            )
        });
        self.format == "marklab.cohort_max_t_calibration"
            && self.version == 1
            && self.input == prepared.input
            && self.population_unit == "whole_patient"
            && self.null == "every_fixed_size_whole_patient_group_assignment_is_equally_likely"
            && self.failure_policy
                == "reject_incomplete_nonfinite_degenerate_or_over_budget_exact_designs"
            && self.patient_count == prepared.records.len()
            && self.group_a_count == prepared.configuration.group_a_count
            && self.group_b_count == prepared.records.len() - self.group_a_count
            && self.family_sizes == prepared.configuration.family_sizes
            && self.alpha.to_bits() == prepared.configuration.alpha.to_bits()
            && self.endpoints == prepared.records[0].endpoints
            && assignment_count == Some(self.exact_assignment_count)
            && self.exact_assignment_count <= prepared.configuration.maximum_assignments
            && self.work.assignments == self.exact_assignment_count
            && self.work.statistic_patient_endpoint_evaluations
                == (self.exact_assignment_count as u64)
                    .checked_mul(self.patient_count as u64)
                    .and_then(|value| value.checked_mul(self.endpoints.len() as u64))
                    .unwrap_or(u64::MAX)
            && expected_comparisons == Some(self.work.assignment_endpoint_comparisons)
            && self.work.assignment_endpoint_comparisons
                <= prepared
                    .configuration
                    .maximum_assignment_endpoint_evaluations
            && self.work.retained_memory_bytes <= prepared.configuration.memory_budget_bytes
            && [
                &self.single_step,
                &self.step_down,
                &self.ordered_gatekeeping_step_down,
            ]
            .iter()
            .all(|method| {
                method.validates(
                    self.endpoints.len(),
                    self.exact_assignment_count,
                    self.alpha,
                )
            })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MethodOutput {
    familywise_error_count: usize,
    familywise_error_rate: f64,
    endpoint_rejection_rates: Vec<f64>,
    calibration_status: String,
}

impl MethodOutput {
    fn validates(&self, endpoint_count: usize, assignment_count: usize, alpha: f64) -> bool {
        let denominator = assignment_count as f64;
        self.familywise_error_rate.is_finite()
            && (0.0..=1.0).contains(&self.familywise_error_rate)
            && self.familywise_error_count <= assignment_count
            && self.familywise_error_rate.to_bits()
                == (self.familywise_error_count as f64 / denominator).to_bits()
            && self.endpoint_rejection_rates.len() == endpoint_count
            && self.endpoint_rejection_rates.iter().all(|value| {
                value.is_finite()
                    && (0.0..=1.0).contains(value)
                    && ((*value * denominator) - (*value * denominator).round()).abs() <= 1e-9
            })
            && self.calibration_status
                == if self.familywise_error_rate <= alpha {
                    "controlled_at_declared_alpha"
                } else {
                    "exceeds_declared_alpha"
                }
    }
}

fn exact_assignment_count(patient_count: usize, group_a_count: usize) -> Option<usize> {
    let selected = group_a_count.min(patient_count.checked_sub(group_a_count)?);
    let mut value = 1_u128;
    for index in 1..=selected {
        value = value.checked_mul((patient_count - selected + index) as u128)? / index as u128;
    }
    usize::try_from(value).ok()
}

fn comparison_work(assignments: usize, endpoints: usize, family_sizes: &[usize]) -> Option<u64> {
    let triangular = |value: usize| {
        (value as u64)
            .checked_mul(value as u64 + 1)
            .map(|product| product / 2)
    };
    let family_triangles = family_sizes
        .iter()
        .try_fold(0_u64, |sum, size| sum.checked_add(triangular(*size)?))?;
    (assignments as u64)
        .checked_mul(assignments as u64)?
        .checked_mul(endpoints as u64 + triangular(endpoints)? + family_triangles)?
        .checked_add((assignments as u64).checked_mul(endpoints as u64)?)
}

impl From<MaxTCalibrationMethodResult> for MethodOutput {
    fn from(result: MaxTCalibrationMethodResult) -> Self {
        Self {
            familywise_error_count: result.familywise_error_count,
            familywise_error_rate: result.familywise_error_rate,
            endpoint_rejection_rates: result.endpoint_rejection_rates,
            calibration_status: result.calibration_status,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkOutput {
    assignments: usize,
    statistic_patient_endpoint_evaluations: u64,
    assignment_endpoint_comparisons: u64,
    retained_memory_bytes: usize,
}

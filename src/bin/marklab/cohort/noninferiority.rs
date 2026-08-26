use std::path::PathBuf;

use clap::ValueEnum;
use marklab_cohort::{
    noninferiority_test, NoninferiorityDirection, NoninferiorityResult, NoninferioritySpec,
};
use serde::Serialize;

use super::{effects::read_effects, publication::publish_json, CohortError};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliDirection {
    HigherIsBetter,
    LowerIsBetter,
}

impl From<CliDirection> for NoninferiorityDirection {
    fn from(value: CliDirection) -> Self {
        match value {
            CliDirection::HigherIsBetter => Self::HigherIsBetter,
            CliDirection::LowerIsBetter => Self::LowerIsBetter,
        }
    }
}

pub(super) fn run(
    input: PathBuf,
    direction: CliDirection,
    margin: f64,
    alpha: f64,
    margin_rationale: String,
    out: PathBuf,
) -> Result<(), CohortError> {
    let effects = read_effects(&input)?;
    let result = noninferiority_test(
        &effects,
        &NoninferioritySpec {
            direction: direction.into(),
            margin,
            alpha,
            margin_rationale,
        },
    )?;
    publish_json(&out, &NoninferiorityOutput::from_result(input, result))
}

#[derive(Debug, Serialize)]
struct NoninferiorityOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: NoninferiorityDesignOutput,
    patient_count: usize,
    estimate: f64,
    standard_error: f64,
    degrees_of_freedom: f64,
    direction: OutputDirection,
    margin: f64,
    null_boundary: f64,
    alpha: f64,
    margin_rationale: String,
    statistic: f64,
    p_value: f64,
    confidence_bound: f64,
    noninferior: bool,
    decision: &'static str,
}

impl NoninferiorityOutput {
    fn from_result(input: PathBuf, result: NoninferiorityResult) -> Self {
        Self {
            format: "marklab.cohort_noninferiority",
            version: 1,
            input,
            design: NoninferiorityDesignOutput {
                analysis_unit: "patient",
                variance_method: "one_sample_student_t",
            },
            patient_count: result.patient_count,
            estimate: result.estimate,
            standard_error: result.standard_error,
            degrees_of_freedom: result.degrees_of_freedom,
            direction: match result.direction {
                NoninferiorityDirection::HigherIsBetter => OutputDirection::HigherIsBetter,
                NoninferiorityDirection::LowerIsBetter => OutputDirection::LowerIsBetter,
            },
            margin: result.margin,
            null_boundary: result.null_boundary,
            alpha: result.alpha,
            margin_rationale: result.margin_rationale,
            statistic: result.statistic,
            p_value: result.p_value,
            confidence_bound: result.confidence_bound,
            noninferior: result.noninferior,
            decision: if result.noninferior {
                "noninferior"
            } else {
                "not_demonstrated"
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct NoninferiorityDesignOutput {
    analysis_unit: &'static str,
    variance_method: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum OutputDirection {
    HigherIsBetter,
    LowerIsBetter,
}

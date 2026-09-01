use std::path::PathBuf;

use clap::ValueEnum;
use marklab_cohort::{
    noninferiority_test, NoninferiorityDirection, NoninferiorityResult, NoninferioritySpec,
    PatientEffect,
};
use serde::{Deserialize, Serialize};

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
    let prepared = prepare(input, direction.into(), margin, alpha, margin_rationale)?;
    publish_json(&out, &execute(&prepared)?)
}

pub(crate) struct PreparedNoninferiority {
    pub(crate) input: PathBuf,
    pub(crate) effects: Vec<PatientEffect>,
    pub(crate) spec: NoninferioritySpec,
}

pub(crate) fn prepare(
    input: PathBuf,
    direction: NoninferiorityDirection,
    margin: f64,
    alpha: f64,
    margin_rationale: String,
) -> Result<PreparedNoninferiority, CohortError> {
    Ok(PreparedNoninferiority {
        effects: read_effects(&input)?,
        input,
        spec: NoninferioritySpec {
            direction,
            margin,
            alpha,
            margin_rationale,
        },
    })
}

pub(crate) fn execute(
    prepared: &PreparedNoninferiority,
) -> Result<NoninferiorityOutput, CohortError> {
    let result = noninferiority_test(&prepared.effects, &prepared.spec)?;
    Ok(NoninferiorityOutput::from_result(
        prepared.input.clone(),
        result,
    ))
}

pub(crate) fn validate_result(
    prepared: &PreparedNoninferiority,
    output: &NoninferiorityOutput,
) -> Result<(), String> {
    let expected_direction = match prepared.spec.direction {
        NoninferiorityDirection::HigherIsBetter => OutputDirection::HigherIsBetter,
        NoninferiorityDirection::LowerIsBetter => OutputDirection::LowerIsBetter,
    };
    let expected_boundary = match prepared.spec.direction {
        NoninferiorityDirection::HigherIsBetter => -prepared.spec.margin,
        NoninferiorityDirection::LowerIsBetter => prepared.spec.margin,
    };
    let finite = [
        output.estimate,
        output.standard_error,
        output.degrees_of_freedom,
        output.margin,
        output.null_boundary,
        output.alpha,
        output.statistic,
        output.p_value,
        output.confidence_bound,
    ]
    .into_iter()
    .all(f64::is_finite);
    let bound_decision = match output.direction {
        OutputDirection::HigherIsBetter => output.confidence_bound > output.null_boundary,
        OutputDirection::LowerIsBetter => output.confidence_bound < output.null_boundary,
    };
    let decision = if output.noninferior {
        "noninferior"
    } else {
        "not_demonstrated"
    };
    if output.format != "marklab.cohort_noninferiority"
        || output.version != 1
        || output.input != prepared.input
        || output.design.analysis_unit != "patient"
        || output.design.variance_method != "one_sample_student_t"
        || output.patient_count != prepared.effects.len()
        || output.direction != expected_direction
        || output.margin != prepared.spec.margin
        || output.null_boundary != expected_boundary
        || output.alpha != prepared.spec.alpha
        || output.margin_rationale != prepared.spec.margin_rationale
        || !finite
        || output.standard_error <= 0.0
        || output.degrees_of_freedom <= 0.0
        || !(0.0..=1.0).contains(&output.p_value)
        || output.noninferior != (output.p_value < output.alpha)
        || output.noninferior != bound_decision
        || output.decision != decision
    {
        return Err(
            "cached noninferiority result identity, bounds, or finite policy differs".into(),
        );
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoninferiorityOutput {
    format: String,
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
    decision: String,
}

impl NoninferiorityOutput {
    fn from_result(input: PathBuf, result: NoninferiorityResult) -> Self {
        Self {
            format: "marklab.cohort_noninferiority".into(),
            version: 1,
            input,
            design: NoninferiorityDesignOutput {
                analysis_unit: "patient".into(),
                variance_method: "one_sample_student_t".into(),
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
                "noninferior".into()
            } else {
                "not_demonstrated".into()
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NoninferiorityDesignOutput {
    analysis_unit: String,
    variance_method: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OutputDirection {
    HigherIsBetter,
    LowerIsBetter,
}

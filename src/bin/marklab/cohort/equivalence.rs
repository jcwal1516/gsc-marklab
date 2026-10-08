use std::path::PathBuf;

use marklab_cohort::{tost_equivalence, PatientEffect, TostEquivalenceResult, TostEquivalenceSpec};
use serde::{Deserialize, Serialize};

use super::{effects::read_effects, publication::publish_json, CohortError};

pub(super) fn run(
    input: PathBuf,
    lower_margin: f64,
    upper_margin: f64,
    alpha: f64,
    margin_rationale: String,
    out: PathBuf,
) -> Result<(), CohortError> {
    let prepared = prepare(input, lower_margin, upper_margin, alpha, margin_rationale)?;
    let result = execute(&prepared)?;
    publish_json(&out, &result)
}

pub(crate) struct PreparedEquivalence {
    pub(crate) input: PathBuf,
    pub(crate) effects: Vec<PatientEffect>,
    pub(crate) spec: TostEquivalenceSpec,
}

pub(crate) fn prepare(
    input: PathBuf,
    lower_margin: f64,
    upper_margin: f64,
    alpha: f64,
    margin_rationale: String,
) -> Result<PreparedEquivalence, CohortError> {
    Ok(PreparedEquivalence {
        effects: read_effects(&input)?,
        input,
        spec: TostEquivalenceSpec {
            lower_margin,
            upper_margin,
            alpha,
            margin_rationale,
        },
    })
}

pub(crate) fn execute(prepared: &PreparedEquivalence) -> Result<EquivalenceOutput, CohortError> {
    let result = tost_equivalence(&prepared.effects, &prepared.spec)?;
    Ok(EquivalenceOutput::from_result(
        prepared.input.clone(),
        result,
    ))
}

pub(crate) fn validate_result(
    prepared: &PreparedEquivalence,
    output: &EquivalenceOutput,
) -> Result<(), String> {
    let finite = [
        output.estimate,
        output.standard_error,
        output.degrees_of_freedom,
        output.lower_margin,
        output.upper_margin,
        output.alpha,
        output.t_lower,
        output.t_upper,
        output.p_lower,
        output.p_upper,
        output.confidence_interval.lower,
        output.confidence_interval.upper,
        output.confidence_interval.level,
    ]
    .into_iter()
    .all(f64::is_finite);
    let p_value_decision = output.p_lower < output.alpha && output.p_upper < output.alpha;
    let interval_decision = output.confidence_interval.lower > output.lower_margin
        && output.confidence_interval.upper < output.upper_margin;
    if output.format != "marklab.cohort_equivalence"
        || output.version != 1
        || output.input != prepared.input
        || output.design.analysis_unit != "patient"
        || output.design.variance_method != "one_sample_student_t"
        || output.patient_count != prepared.effects.len()
        || output.lower_margin != prepared.spec.lower_margin
        || output.upper_margin != prepared.spec.upper_margin
        || output.alpha != prepared.spec.alpha
        || output.margin_rationale != prepared.spec.margin_rationale
        || !finite
        || output.standard_error <= 0.0
        || output.degrees_of_freedom <= 0.0
        || !(0.0..=1.0).contains(&output.p_lower)
        || !(0.0..=1.0).contains(&output.p_upper)
        || output.confidence_interval.lower > output.confidence_interval.upper
        || output.confidence_interval.level != 1.0 - 2.0 * output.alpha
        || output.equivalent != p_value_decision
        || output.equivalent != interval_decision
    {
        return Err("cached equivalence result identity, bounds, or finite policy differs".into());
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EquivalenceOutput {
    format: String,
    version: u32,
    input: PathBuf,
    design: EquivalenceDesignOutput,
    patient_count: usize,
    estimate: f64,
    standard_error: f64,
    degrees_of_freedom: f64,
    lower_margin: f64,
    upper_margin: f64,
    alpha: f64,
    margin_rationale: String,
    t_lower: f64,
    t_upper: f64,
    p_lower: f64,
    p_upper: f64,
    confidence_interval: IntervalOutput,
    equivalent: bool,
}

impl EquivalenceOutput {
    fn from_result(input: PathBuf, result: TostEquivalenceResult) -> Self {
        Self {
            format: "marklab.cohort_equivalence".into(),
            version: 1,
            input,
            design: EquivalenceDesignOutput {
                analysis_unit: "patient".into(),
                variance_method: "one_sample_student_t".into(),
            },
            patient_count: result.patient_count,
            estimate: result.estimate,
            standard_error: result.standard_error,
            degrees_of_freedom: result.degrees_of_freedom,
            lower_margin: result.lower_margin,
            upper_margin: result.upper_margin,
            alpha: result.alpha,
            margin_rationale: result.margin_rationale,
            t_lower: result.t_lower,
            t_upper: result.t_upper,
            p_lower: result.p_lower,
            p_upper: result.p_upper,
            confidence_interval: IntervalOutput {
                lower: result.confidence_interval.lower,
                upper: result.confidence_interval.upper,
                level: result.confidence_interval.level,
            },
            equivalent: result.equivalent,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EquivalenceDesignOutput {
    analysis_unit: String,
    variance_method: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct IntervalOutput {
    lower: f64,
    upper: f64,
    level: f64,
}

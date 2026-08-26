use std::path::PathBuf;

use marklab_cohort::{tost_equivalence, TostEquivalenceResult, TostEquivalenceSpec};
use serde::Serialize;

use super::{effects::read_effects, publication::publish_json, CohortError};

pub(super) fn run(
    input: PathBuf,
    lower_margin: f64,
    upper_margin: f64,
    alpha: f64,
    margin_rationale: String,
    out: PathBuf,
) -> Result<(), CohortError> {
    let effects = read_effects(&input)?;
    let result = tost_equivalence(
        &effects,
        &TostEquivalenceSpec {
            lower_margin,
            upper_margin,
            alpha,
            margin_rationale,
        },
    )?;
    publish_json(&out, &EquivalenceOutput::from_result(input, result))
}

#[derive(Debug, Serialize)]
struct EquivalenceOutput {
    format: &'static str,
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
            format: "marklab.cohort_equivalence",
            version: 1,
            input,
            design: EquivalenceDesignOutput {
                analysis_unit: "patient",
                variance_method: "one_sample_student_t",
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

#[derive(Debug, Serialize)]
struct EquivalenceDesignOutput {
    analysis_unit: &'static str,
    variance_method: &'static str,
}

#[derive(Debug, Serialize)]
struct IntervalOutput {
    lower: f64,
    upper: f64,
    level: f64,
}

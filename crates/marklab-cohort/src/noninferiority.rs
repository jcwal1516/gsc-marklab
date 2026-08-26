use statrs::distribution::{ContinuousCDF, StudentsT};

use super::{
    equivalence::{validate_effects, PatientEffect},
    numeric::mean_standard_error,
    CohortInferenceError,
};

/// Favorable direction and corresponding noninferiority boundary convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoninferiorityDirection {
    /// Larger effects are better; the null boundary is `-margin`.
    HigherIsBetter,
    /// Smaller effects are better; the null boundary is `+margin`.
    LowerIsBetter,
}

/// Frozen one-sided patient-level noninferiority design.
#[derive(Clone, Debug)]
pub struct NoninferioritySpec {
    /// Explicit favorable direction.
    pub direction: NoninferiorityDirection,
    /// Finite positive noninferiority margin magnitude.
    pub margin: f64,
    /// One-sided alpha, finite and below one half.
    pub alpha: f64,
    /// Non-empty prespecified margin rationale reference.
    pub margin_rationale: String,
}

/// Patient-level Student-t noninferiority result.
#[derive(Clone, Debug, PartialEq)]
pub struct NoninferiorityResult {
    /// Independent patient count.
    pub patient_count: usize,
    /// Stable mean patient effect.
    pub estimate: f64,
    /// Sample standard error of the mean.
    pub standard_error: f64,
    /// Student-t degrees of freedom.
    pub degrees_of_freedom: f64,
    /// Favorable direction.
    pub direction: NoninferiorityDirection,
    /// Positive margin magnitude.
    pub margin: f64,
    /// Signed null boundary.
    pub null_boundary: f64,
    /// One-sided alpha.
    pub alpha: f64,
    /// Margin rationale reference.
    pub margin_rationale: String,
    /// Favorable-direction test statistic.
    pub statistic: f64,
    /// Upper-tail p-value for the favorable-direction statistic.
    pub p_value: f64,
    /// Lower bound when higher is better; upper bound when lower is better.
    pub confidence_bound: f64,
    /// True only when p-value and matching one-sided bound reject the null.
    pub noninferior: bool,
}

/// Test one mean patient effect against a prespecified directional noninferiority margin.
pub fn noninferiority_test(
    effects: &[PatientEffect],
    spec: &NoninferioritySpec,
) -> Result<NoninferiorityResult, CohortInferenceError> {
    validate_spec(spec)?;
    let values = validate_effects(effects)?;
    let summary = mean_standard_error(&values)?;
    let distribution = StudentsT::new(0.0, 1.0, summary.degrees_of_freedom).map_err(|error| {
        CohortInferenceError::NumericalFailure(format!(
            "failed to construct Student-t distribution: {error}"
        ))
    })?;
    let critical = distribution.inverse_cdf(1.0 - spec.alpha);
    let (null_boundary, statistic, confidence_bound, bound_noninferior) = match spec.direction {
        NoninferiorityDirection::HigherIsBetter => {
            let boundary = -spec.margin;
            let statistic = (summary.mean - boundary) / summary.standard_error;
            let bound = summary.mean - critical * summary.standard_error;
            (boundary, statistic, bound, bound > boundary)
        }
        NoninferiorityDirection::LowerIsBetter => {
            let boundary = spec.margin;
            let statistic = (boundary - summary.mean) / summary.standard_error;
            let bound = summary.mean + critical * summary.standard_error;
            (boundary, statistic, bound, bound < boundary)
        }
    };
    let p_value = 1.0 - distribution.cdf(statistic);
    if [critical, statistic, confidence_bound, p_value]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(CohortInferenceError::NumericalFailure(
            "noninferiority test produced a non-finite result".into(),
        ));
    }
    let noninferior = p_value < spec.alpha;
    if noninferior != bound_noninferior {
        return Err(CohortInferenceError::NumericalFailure(
            "noninferiority p-value and matching confidence-bound decisions disagree".into(),
        ));
    }
    Ok(NoninferiorityResult {
        patient_count: summary.count,
        estimate: summary.mean,
        standard_error: summary.standard_error,
        degrees_of_freedom: summary.degrees_of_freedom,
        direction: spec.direction,
        margin: spec.margin,
        null_boundary,
        alpha: spec.alpha,
        margin_rationale: spec.margin_rationale.clone(),
        statistic,
        p_value,
        confidence_bound,
        noninferior,
    })
}

fn validate_spec(spec: &NoninferioritySpec) -> Result<(), CohortInferenceError> {
    if !(spec.margin.is_finite() && spec.margin > 0.0) {
        return Err(CohortInferenceError::InvalidInput(
            "noninferiority margin must be finite and positive".into(),
        ));
    }
    if !(spec.alpha.is_finite() && spec.alpha > 0.0 && spec.alpha < 0.5) {
        return Err(CohortInferenceError::InvalidInput(
            "noninferiority alpha must be finite and strictly between zero and one half".into(),
        ));
    }
    if spec.margin_rationale.trim().is_empty()
        || spec.margin_rationale.trim() != spec.margin_rationale
    {
        return Err(CohortInferenceError::InvalidInput(
            "margin rationale must be non-empty without surrounding whitespace".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effects() -> Vec<PatientEffect> {
        [-0.1, 0.0, 0.1, 0.0]
            .into_iter()
            .enumerate()
            .map(|(index, effect)| PatientEffect {
                patient_id: format!("p-{index}"),
                effect,
            })
            .collect()
    }

    #[test]
    fn both_directions_match_symmetric_fixture() {
        for direction in [
            NoninferiorityDirection::HigherIsBetter,
            NoninferiorityDirection::LowerIsBetter,
        ] {
            let result = noninferiority_test(
                &effects(),
                &NoninferioritySpec {
                    direction,
                    margin: 0.2,
                    alpha: 0.05,
                    margin_rationale: "protocol-ni-margin-v1".into(),
                },
            )
            .expect("noninferiority result");
            assert!(result.noninferior);
            assert!(result.p_value < 0.05);
        }
    }
}

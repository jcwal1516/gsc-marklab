use std::collections::HashSet;

use statrs::distribution::{ContinuousCDF, StudentsT};

use super::{numeric::mean_standard_error, CohortInferenceError, MAXIMUM_PATIENTS};

/// One prespecified paired or otherwise design-valid patient-level effect.
#[derive(Clone, Debug)]
pub struct PatientEffect {
    /// Stable patient identifier.
    pub patient_id: String,
    /// Finite effect using the caller-declared sign convention.
    pub effect: f64,
}

/// Frozen two-one-sided-tests equivalence design.
#[derive(Clone, Debug)]
pub struct TostEquivalenceSpec {
    /// Strict lower equivalence margin.
    pub lower_margin: f64,
    /// Strict upper equivalence margin.
    pub upper_margin: f64,
    /// One-sided alpha, finite and below one half.
    pub alpha: f64,
    /// Non-empty prespecified margin rationale reference.
    pub margin_rationale: String,
}

/// Matching two-sided `1-2*alpha` confidence interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquivalenceInterval {
    /// Lower confidence bound.
    pub lower: f64,
    /// Upper confidence bound.
    pub upper: f64,
    /// Confidence level `1-2*alpha`.
    pub level: f64,
}

/// Patient-level Student-t TOST result.
#[derive(Clone, Debug, PartialEq)]
pub struct TostEquivalenceResult {
    /// Independent patient count.
    pub patient_count: usize,
    /// Stable mean patient effect.
    pub estimate: f64,
    /// Sample standard error of the mean.
    pub standard_error: f64,
    /// Student-t degrees of freedom.
    pub degrees_of_freedom: f64,
    /// Prespecified lower margin.
    pub lower_margin: f64,
    /// Prespecified upper margin.
    pub upper_margin: f64,
    /// One-sided alpha.
    pub alpha: f64,
    /// Margin rationale reference.
    pub margin_rationale: String,
    /// Lower-margin test statistic.
    pub t_lower: f64,
    /// Upper-margin test statistic.
    pub t_upper: f64,
    /// Upper-tail p-value rejecting effects at or below the lower margin.
    pub p_lower: f64,
    /// Lower-tail p-value rejecting effects at or above the upper margin.
    pub p_upper: f64,
    /// Matching confidence interval.
    pub confidence_interval: EquivalenceInterval,
    /// True only when both one-sided nulls are rejected.
    pub equivalent: bool,
}

/// Test whether a prespecified mean patient effect lies strictly inside fixed margins.
pub fn tost_equivalence(
    effects: &[PatientEffect],
    spec: &TostEquivalenceSpec,
) -> Result<TostEquivalenceResult, CohortInferenceError> {
    validate_spec(spec)?;
    let values = validate_effects(effects)?;
    let summary = mean_standard_error(&values)?;
    let distribution = StudentsT::new(0.0, 1.0, summary.degrees_of_freedom).map_err(|error| {
        CohortInferenceError::NumericalFailure(format!(
            "failed to construct Student-t distribution: {error}"
        ))
    })?;
    let t_lower = (summary.mean - spec.lower_margin) / summary.standard_error;
    let t_upper = (summary.mean - spec.upper_margin) / summary.standard_error;
    let p_lower = 1.0 - distribution.cdf(t_lower);
    let p_upper = distribution.cdf(t_upper);
    let critical = distribution.inverse_cdf(1.0 - spec.alpha);
    let confidence_interval = EquivalenceInterval {
        lower: summary.mean - critical * summary.standard_error,
        upper: summary.mean + critical * summary.standard_error,
        level: 1.0 - 2.0 * spec.alpha,
    };
    if [
        t_lower,
        t_upper,
        p_lower,
        p_upper,
        critical,
        confidence_interval.lower,
        confidence_interval.upper,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(CohortInferenceError::NumericalFailure(
            "TOST produced a non-finite result".into(),
        ));
    }
    let equivalent = p_lower < spec.alpha && p_upper < spec.alpha;
    let interval_equivalent = confidence_interval.lower > spec.lower_margin
        && confidence_interval.upper < spec.upper_margin;
    if equivalent != interval_equivalent {
        return Err(CohortInferenceError::NumericalFailure(
            "TOST p-value and matching confidence-interval decisions disagree".into(),
        ));
    }
    Ok(TostEquivalenceResult {
        patient_count: summary.count,
        estimate: summary.mean,
        standard_error: summary.standard_error,
        degrees_of_freedom: summary.degrees_of_freedom,
        lower_margin: spec.lower_margin,
        upper_margin: spec.upper_margin,
        alpha: spec.alpha,
        margin_rationale: spec.margin_rationale.clone(),
        t_lower,
        t_upper,
        p_lower,
        p_upper,
        confidence_interval,
        equivalent,
    })
}

pub(crate) fn validate_effects(
    effects: &[PatientEffect],
) -> Result<Vec<f64>, CohortInferenceError> {
    if effects.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "equivalence input exceeds the {MAXIMUM_PATIENTS}-patient limit"
        )));
    }
    let mut patients = HashSet::with_capacity(effects.len());
    let mut values = Vec::with_capacity(effects.len());
    for effect in effects {
        if effect.patient_id.trim().is_empty() || effect.patient_id.trim() != effect.patient_id {
            return Err(CohortInferenceError::InvalidInput(
                "patient_id must be non-empty without surrounding whitespace".into(),
            ));
        }
        if !patients.insert(effect.patient_id.as_str()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate patient effect: {}",
                effect.patient_id
            )));
        }
        if !effect.effect.is_finite() {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} effect must be finite",
                effect.patient_id
            )));
        }
        values.push(effect.effect);
    }
    Ok(values)
}

fn validate_spec(spec: &TostEquivalenceSpec) -> Result<(), CohortInferenceError> {
    if !(spec.lower_margin.is_finite()
        && spec.upper_margin.is_finite()
        && spec.lower_margin < spec.upper_margin)
    {
        return Err(CohortInferenceError::InvalidInput(
            "equivalence margins must be finite with lower_margin < upper_margin".into(),
        ));
    }
    if !(spec.alpha.is_finite() && spec.alpha > 0.0 && spec.alpha < 0.5) {
        return Err(CohortInferenceError::InvalidInput(
            "equivalence alpha must be finite and strictly between zero and one half".into(),
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

    #[test]
    fn symmetric_fixture_is_equivalent() {
        let effects = [-0.1, 0.0, 0.1, 0.0]
            .into_iter()
            .enumerate()
            .map(|(index, effect)| PatientEffect {
                patient_id: format!("p-{index}"),
                effect,
            })
            .collect::<Vec<_>>();
        let result = tost_equivalence(
            &effects,
            &TostEquivalenceSpec {
                lower_margin: -0.2,
                upper_margin: 0.2,
                alpha: 0.05,
                margin_rationale: "protocol-margin-v1".into(),
            },
        )
        .expect("TOST result");
        assert_eq!(result.estimate, 0.0);
        assert_eq!(result.degrees_of_freedom, 3.0);
        assert!((result.standard_error - 0.040_824_829_046_386_304).abs() < 1e-15);
        assert!((result.t_lower - 4.898_979_485_566_356).abs() < 1e-14);
        assert!((result.p_lower - 0.008_138_301_729_714_28).abs() < 1e-15);
        assert!((result.p_upper - 0.008_138_301_729_714_28).abs() < 1e-15);
        assert!((result.confidence_interval.lower + 0.096_075_659_909_800_98).abs() < 1e-14);
        assert!((result.confidence_interval.upper - 0.096_075_659_909_800_98).abs() < 1e-14);
        assert!(result.equivalent);
        assert!(result.confidence_interval.lower > -0.2);
        assert!(result.confidence_interval.upper < 0.2);
    }
}

use crate::{
    comparison::curves::max_abs_standardized_difference,
    errors::{MarklabError, Result},
    output::{CurveTestAvailability, CurveTestResult},
};

/// Compare two curves against an optional maximum standardized-difference margin.
///
/// A zero margin is accepted and requires an exact match under the
/// `max_abs_standardized_difference` metric. This is a descriptive threshold
/// comparison, not an inferential equivalence test.
pub fn curve_margin_assessment(
    comparison_name: &str,
    a: &[f64],
    b: &[f64],
    margin: Option<f64>,
) -> Result<CurveTestResult> {
    let statistic = max_abs_standardized_difference(a, b)?;
    validate_margin(margin)?;

    let (within_margin, interpretation) = match margin {
        Some(margin) => (
            Some(statistic <= margin),
            if statistic <= margin {
                "curve distance is within the requested descriptive margin".into()
            } else {
                "curve distance is outside the requested descriptive margin".into()
            },
        ),
        None => (
            None,
            "margin assessment is unavailable without a prespecified descriptive margin".into(),
        ),
    };

    Ok(CurveTestResult {
        comparison_name: comparison_name.to_owned(),
        metric: "max_abs_standardized_difference".into(),
        availability: CurveTestAvailability::Available,
        statistic: Some(statistic),
        unavailable_reason: None,
        p_difference: None,
        margin,
        within_margin,
        interpretation,
    })
}

fn validate_margin(margin: Option<f64>) -> Result<()> {
    match margin {
        Some(margin) if !margin.is_finite() || margin < 0.0 => Err(MarklabError::Config(
            "curve comparison margin must be finite and non-negative".into(),
        )),
        _ => Ok(()),
    }
}

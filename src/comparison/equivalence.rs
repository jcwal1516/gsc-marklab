use crate::{
    comparison::curves::max_abs_standardized_difference,
    errors::{MarklabError, Result},
    output::CurveTestResult,
};

/// Compare two curves against an optional maximum standardized-difference margin.
///
/// A zero margin is accepted and represents exact equivalence under the
/// `max_abs_standardized_difference` metric.
pub fn curve_equivalence_test(
    comparison_name: &str,
    a: &[f64],
    b: &[f64],
    margin: Option<f64>,
) -> Result<CurveTestResult> {
    let statistic = max_abs_standardized_difference(a, b)?;
    validate_margin(margin)?;

    let (equivalent, interpretation) = match margin {
        Some(margin) => (
            Some(statistic <= margin),
            if statistic <= margin {
                "curves are equivalent within the requested margin".into()
            } else {
                "curves are not equivalent within the requested margin".into()
            },
        ),
        None => (
            None,
            "equivalence assessment is non-confirmatory without a prespecified margin".into(),
        ),
    };

    Ok(CurveTestResult {
        comparison_name: comparison_name.to_owned(),
        metric: "max_abs_standardized_difference".into(),
        statistic,
        p_difference: None,
        equivalence_margin: margin,
        p_equivalence: None,
        equivalent,
        interpretation,
    })
}

fn validate_margin(margin: Option<f64>) -> Result<()> {
    match margin {
        Some(margin) if !margin.is_finite() || margin < 0.0 => Err(MarklabError::Config(
            "curve equivalence margin must be finite and non-negative".into(),
        )),
        _ => Ok(()),
    }
}

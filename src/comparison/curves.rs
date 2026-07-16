use crate::errors::{MmrspaceError, Result};

pub fn max_abs_standardized_difference(a: &[f64], b: &[f64]) -> Result<f64> {
    validate_curves(a, b)?;

    Ok(a.iter()
        .zip(b)
        .map(|(left, right)| {
            let denominator = left.abs().max(right.abs()).max(1.0);
            (left - right).abs() / denominator
        })
        .fold(0.0_f64, f64::max))
}

pub(crate) fn validate_curves(a: &[f64], b: &[f64]) -> Result<()> {
    if a.is_empty() || b.is_empty() {
        return Err(MmrspaceError::Validation(
            "curve comparison requires non-empty curves".into(),
        ));
    }
    if a.len() != b.len() {
        return Err(MmrspaceError::Validation(format!(
            "curve lengths must match for comparison: {} vs {}",
            a.len(),
            b.len()
        )));
    }
    if !a.iter().chain(b.iter()).all(|value| value.is_finite()) {
        return Err(MmrspaceError::Validation(
            "curve comparison requires finite values".into(),
        ));
    }
    Ok(())
}

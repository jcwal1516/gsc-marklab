use serde::{Deserialize, Serialize};

use crate::errors::{MmrspaceError, Result};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tail {
    OneSidedHigh,
    OneSidedLow,
    TwoSided,
}

pub fn permutation_p_value(
    observed: f64,
    null_values: &[f64],
    tail: Tail,
    alpha: f64,
) -> Result<f64> {
    if !(alpha.is_finite() && 0.0 < alpha && alpha < 1.0) {
        return Err(MmrspaceError::Validation(
            "permutation-test alpha must be finite and strictly between zero and one".into(),
        ));
    }
    if !observed.is_finite() {
        return Err(MmrspaceError::Compute(
            "observed permutation statistic is not finite".into(),
        ));
    }
    if null_values.is_empty() {
        return Err(MmrspaceError::Validation(
            "permutation test requires at least one null statistic".into(),
        ));
    }
    if null_values.iter().any(|value| !value.is_finite()) {
        return Err(MmrspaceError::Compute(
            "permutation null statistics contain a non-finite value".into(),
        ));
    }

    let denominator = null_values.len() + 1;
    if tail == Tail::TwoSided && (denominator as f64) < 2.0 / alpha {
        return Err(MmrspaceError::Validation(format!(
            "equal-tail permutation test requires B + 1 >= 2 / alpha (got {denominator} and alpha {alpha})"
        )));
    }

    let p_value = match tail {
        Tail::OneSidedHigh => {
            let count = null_values
                .iter()
                .filter(|value| **value >= observed)
                .count();
            (count as f64 + 1.0) / denominator as f64
        }
        Tail::OneSidedLow => {
            let count = null_values
                .iter()
                .filter(|value| **value <= observed)
                .count();
            (count as f64 + 1.0) / denominator as f64
        }
        Tail::TwoSided => {
            let lower_count = null_values
                .iter()
                .filter(|value| **value <= observed)
                .count();
            let upper_count = null_values
                .iter()
                .filter(|value| **value >= observed)
                .count();
            let p_lower = (lower_count as f64 + 1.0) / denominator as f64;
            let p_upper = (upper_count as f64 + 1.0) / denominator as f64;
            (2.0 * p_lower.min(p_upper)).min(1.0)
        }
    };

    Ok(p_value)
}

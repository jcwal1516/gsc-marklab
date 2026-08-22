use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tail {
    OneSidedHigh,
    OneSidedLow,
    TwoSided,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermutationTestSpec {
    pub tail: Tail,
    pub minimum_permutations: usize,
}

impl PermutationTestSpec {
    pub const fn new(tail: Tail, minimum_permutations: usize) -> Self {
        Self {
            tail,
            minimum_permutations,
        }
    }

    pub fn for_alpha(tail: Tail, alpha: f64) -> Result<Self> {
        if !(alpha.is_finite() && 0.0 < alpha && alpha < 1.0) {
            return Err(MarklabError::Validation(
                "permutation-test alpha must be finite and strictly between zero and one".into(),
            ));
        }
        let numerator = if tail == Tail::TwoSided { 2.0 } else { 1.0 };
        let minimum_permutations = (numerator / alpha).ceil() as usize - 1;
        Ok(Self::new(tail, minimum_permutations))
    }
}

pub fn permutation_p_value(
    observed: f64,
    null_values: &[f64],
    tail: Tail,
    alpha: f64,
) -> Result<f64> {
    let specification = PermutationTestSpec::for_alpha(tail, alpha)?;
    permutation_p_value_with_spec(observed, null_values, specification)
}

pub fn permutation_p_value_with_spec(
    observed: f64,
    null_values: &[f64],
    specification: PermutationTestSpec,
) -> Result<f64> {
    if !observed.is_finite() {
        return Err(MarklabError::Compute(
            "observed permutation statistic is not finite".into(),
        ));
    }
    if specification.minimum_permutations == 0 {
        return Err(MarklabError::Validation(
            "permutation test minimum must be greater than zero".into(),
        ));
    }
    if null_values.len() < specification.minimum_permutations {
        return Err(MarklabError::Validation(format!(
            "permutation test requires at least {} null statistics (got {})",
            specification.minimum_permutations,
            null_values.len()
        )));
    }
    if null_values.iter().any(|value| !value.is_finite()) {
        return Err(MarklabError::Compute(
            "permutation null statistics contain a non-finite value".into(),
        ));
    }

    let denominator = null_values.len() + 1;
    let p_value = match specification.tail {
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

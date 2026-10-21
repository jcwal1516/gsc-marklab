#![forbid(unsafe_code)]
//! Shared numerical routines for Marklab: compensated summation, dense linear algebra,
//! covariance kernels, and extreme-rank-length envelopes.

use thiserror::Error;

pub mod dense;
mod envelope;
pub mod kernel;
mod primitives;
mod probability;
pub mod summation;

pub use envelope::{extreme_rank_length_envelope, ExtremeRankLengthEnvelope};
pub use primitives::{evaluate_stable_primitives, StablePrimitivesResult, StablePrimitivesSpec};
pub use probability::{clipped_logit, sigmoid};

#[derive(Debug, Error)]
pub enum NumericsError {
    #[error("invalid numerical primitive input: {0}")]
    Invalid(String),
    #[error("numerical primitive resource limit exceeded: {0}")]
    Resource(String),
    #[error("numerical primitive failure: {0}")]
    Numerical(String),
}

/// Whether `left` and `right` agree within `tolerance` relative to the larger magnitude,
/// with an absolute floor of `tolerance` near zero. Non-finite values never agree.
pub fn relative_eq(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance * left.abs().max(right.abs()).max(1.0)
}

/// Whether every entry is finite; an empty slice is finite.
pub fn all_finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::relative_eq;

    #[test]
    fn relative_eq_scales_with_magnitude_and_rejects_non_finite_values() {
        assert!(relative_eq(1.0e6, 1.0e6 + 0.5, 1.0e-6));
        assert!(!relative_eq(1.0e6, 1.0e6 + 2.0, 1.0e-6));
        assert!(relative_eq(0.0, 1.0e-13, 1.0e-12));
        assert!(!relative_eq(f64::INFINITY, f64::INFINITY, 1.0));
        assert!(!relative_eq(f64::NAN, 0.0, 1.0));
    }
}

use crate::{BayesError, DiagnosticPolicy, McseScalarSummary, PosteriorDiagnostics, ScalarSummary};

#[derive(Clone, Copy)]
pub(crate) enum SummarySupport {
    Real,
    Unit,
    Difference,
    Positive,
}

pub(crate) use marklab_core::backend::is_sha256;

/// A user-supplied name or identifier: nonempty, at most 128 bytes, without surrounding
/// whitespace and without control characters.
pub(crate) fn is_exact_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

pub(crate) use marklab_numerics::all_finite;

pub(crate) fn diagnostics_satisfy_policy(
    diagnostics: &PosteriorDiagnostics,
    policy: &DiagnosticPolicy,
) -> bool {
    diagnostics.prior_predictive_finite
        && diagnostics.posterior_finite
        && diagnostics.constraints_valid
        && diagnostics.identifiability_checks_passed
        && diagnostics.r_hat <= policy.maximum_r_hat
        && diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && diagnostics.ess_tail >= policy.minimum_tail_ess
        && diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && diagnostics.divergences <= policy.maximum_divergences
        && diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits
}

pub(crate) fn validate_scalar_summary(
    summary: &ScalarSummary,
    support: SummarySupport,
    invalid_message: &str,
) -> Result<(), BayesError> {
    if !all_finite(&[
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]) || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
        || matches!(support, SummarySupport::Unit)
            && (!(0.0..=1.0).contains(&summary.mean)
                || !(0.0..=1.0).contains(&summary.interval_lower)
                || !(0.0..=1.0).contains(&summary.interval_upper))
        || matches!(support, SummarySupport::Difference)
            && (!(-1.0..=1.0).contains(&summary.mean)
                || !(-1.0..=1.0).contains(&summary.interval_lower)
                || !(-1.0..=1.0).contains(&summary.interval_upper))
        || matches!(support, SummarySupport::Positive)
            && (summary.mean <= 0.0 || summary.interval_lower < 0.0)
    {
        return Err(BayesError::WorkerContract(invalid_message.into()));
    }
    Ok(())
}

/// Validates a summary as [`validate_scalar_summary`] does and requires a finite, non-negative
/// MCSE (zero only for constant draws).
pub(crate) fn validate_mcse_summary(
    summary: &McseScalarSummary,
    support: SummarySupport,
    invalid_message: &str,
) -> Result<(), BayesError> {
    if !summary.mcse_mean.is_finite() || summary.mcse_mean < 0.0 {
        return Err(BayesError::WorkerContract(invalid_message.into()));
    }
    validate_scalar_summary(&summary.scalar(), support, invalid_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_identity_requires_exact_lowercase_ascii_hex() {
        assert!(is_sha256(&"0123456789abcdef".repeat(4)));
        assert!(!is_sha256(&"0123456789ABCDEF".repeat(4)));
        assert!(!is_sha256(&"0".repeat(63)));
        assert!(!is_sha256(&format!("{}g", "0".repeat(63))));
    }

    #[test]
    fn exact_names_reject_empty_padded_oversized_and_control_strings() {
        assert!(is_exact_name("patient-01"));
        assert!(is_exact_name("site A"));
        assert!(is_exact_name(&"x".repeat(128)));
        for invalid in [
            "",
            " patient",
            "patient ",
            "pa\ttient",
            "patient\u{7f}",
            "pa\ntient",
        ] {
            assert!(!is_exact_name(invalid), "{invalid:?}");
        }
        assert!(!is_exact_name(&"x".repeat(129)));
    }

    #[test]
    fn finite_slice_accepts_empty_and_rejects_each_nonfinite_class() {
        assert!(all_finite(&[]));
        assert!(all_finite(&[-0.0, f64::MIN, f64::MAX]));
        assert!(!all_finite(&[f64::NAN]));
        assert!(!all_finite(&[f64::INFINITY]));
        assert!(!all_finite(&[f64::NEG_INFINITY]));
    }

    #[test]
    fn scalar_summary_supports_preserve_bounds_and_error_context() {
        let cases = [
            (2.0, 1.0, 3.0, SummarySupport::Real, true),
            (0.5, 0.0, 1.0, SummarySupport::Unit, true),
            (0.0, -1.0, 1.0, SummarySupport::Difference, true),
            (1.0, 0.0, 2.0, SummarySupport::Positive, true),
            (1.1, 0.0, 1.1, SummarySupport::Unit, false),
            (0.0, -1.0, 1.1, SummarySupport::Difference, false),
            (1.0, -f64::EPSILON, 2.0, SummarySupport::Positive, false),
        ];

        for (mean, interval_lower, interval_upper, support, expected_valid) in cases {
            let summary = ScalarSummary {
                mean,
                sd: 0.5,
                interval_lower,
                interval_upper,
            };
            let result = validate_scalar_summary(&summary, support, "caller-specific error");
            assert_eq!(result.is_ok(), expected_valid);
            if !expected_valid {
                assert!(matches!(
                    result,
                    Err(BayesError::WorkerContract(message)) if message == "caller-specific error"
                ));
            }
        }
    }

    #[test]
    fn mcse_summary_requires_a_finite_non_negative_standard_error() {
        for (mcse_mean, expected_valid) in [
            (0.01, true),
            (0.0, true),
            (-0.01, false),
            (f64::NAN, false),
            (f64::INFINITY, false),
        ] {
            let summary = McseScalarSummary {
                mean: 0.5,
                sd: 0.1,
                interval_lower: 0.3,
                interval_upper: 0.7,
                mcse_mean,
            };
            let result = validate_mcse_summary(&summary, SummarySupport::Unit, "error");
            assert_eq!(result.is_ok(), expected_valid, "mcse_mean={mcse_mean}");
        }
    }
}

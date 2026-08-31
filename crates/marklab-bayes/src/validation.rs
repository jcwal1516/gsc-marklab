use crate::{BayesError, SarScalarSummary};

#[derive(Clone, Copy)]
pub(crate) enum SummarySupport {
    Real,
    Unit,
    Difference,
    Positive,
}

pub(crate) fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn all_finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

pub(crate) fn validate_scalar_summary(
    summary: &SarScalarSummary,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_identity_requires_exact_lowercase_ascii_hex() {
        assert!(is_lower_hex_sha256(&"0123456789abcdef".repeat(4)));
        assert!(!is_lower_hex_sha256(&"0123456789ABCDEF".repeat(4)));
        assert!(!is_lower_hex_sha256(&"0".repeat(63)));
        assert!(!is_lower_hex_sha256(&format!("{}g", "0".repeat(63))));
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
            let summary = SarScalarSummary {
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
}

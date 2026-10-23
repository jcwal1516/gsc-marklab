//! Common finite-state SBC consistency rules; model identities and counts stay in each protocol.

use std::collections::BTreeSet;

pub(super) fn enumeration_valid(
    states: u64,
    zero_log_normalizer: f64,
    types: usize,
    points: usize,
) -> bool {
    let expected_states = u32::try_from(points)
        .ok()
        .and_then(|points| (types as u64).checked_pow(points));
    let expected_normalizer = points as f64 * (types as f64).ln();
    expected_states == Some(states)
        && zero_log_normalizer.is_finite()
        && (zero_log_normalizer - expected_normalizer).abs() <= 1e-12
}

pub(super) fn dispositions_valid<'a>(
    completed: impl Iterator<Item = u32>,
    failures: impl Iterator<Item = (u32, &'a str)>,
    expected: u32,
) -> bool {
    let mut seen = BTreeSet::new();
    for index in completed {
        if index >= expected || !seen.insert(index) {
            return false;
        }
    }
    for (index, reason) in failures {
        if index >= expected || !seen.insert(index) || reason.is_empty() {
            return false;
        }
    }
    seen.len() == expected as usize
}

pub(super) fn parameters_valid<'a>(
    parameters: impl ExactSizeIterator<Item = (&'a str, f64, u64)>,
    names: &[String],
    completed_draws: u64,
) -> bool {
    parameters.len() == names.len()
        && parameters
            .zip(names)
            .all(|((name, truth, rank), expected)| {
                name == expected && truth.is_finite() && rank <= completed_draws
            })
}

/// Each diagnostic supplies its name, histogram, and [uniformity p, coverage, mean rank].
pub(super) fn diagnostics_valid<'a>(
    diagnostics: impl ExactSizeIterator<Item = (&'a str, &'a [u64], [f64; 3])>,
    names: &[String],
    rank_bins: u32,
    completed: usize,
) -> bool {
    diagnostics.len() == names.len()
        && diagnostics
            .zip(names)
            .all(|((name, histogram, values), expected)| {
                name == expected
                    && histogram.len() == rank_bins as usize
                    && histogram
                        .iter()
                        .try_fold(0_u64, |sum, count| sum.checked_add(*count))
                        == Some(completed as u64)
                    && values
                        .into_iter()
                        .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            })
}

pub(super) fn calibration_passes(
    mut diagnostics: impl Iterator<Item = (f64, f64)>,
    minimum_p_value: f64,
    minimum_coverage: f64,
    maximum_coverage: f64,
) -> bool {
    diagnostics.all(|(p_value, coverage)| {
        p_value >= minimum_p_value && (minimum_coverage..=maximum_coverage).contains(&coverage)
    })
}

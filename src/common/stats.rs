/// Median of a finite, ascending slice, averaging the two middle values.
///
/// Returns `None` for empty, non-finite, or unsorted input.
pub(crate) fn median_sorted_average_even(sorted_values: &[f64]) -> Option<f64> {
    if sorted_values.is_empty()
        || sorted_values.iter().any(|value| !value.is_finite())
        || sorted_values.windows(2).any(|pair| pair[0] > pair[1])
    {
        return None;
    }

    let midpoint = sorted_values.len() / 2;
    if sorted_values.len().is_multiple_of(2) {
        Some(sorted_values[midpoint - 1] * 0.5 + sorted_values[midpoint] * 0.5)
    } else {
        Some(sorted_values[midpoint])
    }
}

/// Sort finite values in place and return the average-even median.
pub(crate) fn median_average_even(values: &mut [f64]) -> Option<f64> {
    if values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    values.sort_by(f64::total_cmp);
    median_sorted_average_even(values)
}

/// Return the average-even median after dropping non-finite observations.
pub(crate) fn median_ignoring_nonfinite(values: &[f64]) -> Option<f64> {
    let mut finite = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    median_average_even(&mut finite)
}

/// Arithmetic mean that rejects empty input and any non-finite observation.
pub(crate) fn mean_all_finite(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0_usize;
    for value in values {
        if !value.is_finite() {
            return None;
        }
        sum += value;
        count += 1;
    }
    let mean = (count > 0).then_some(sum / count as f64)?;
    mean.is_finite().then_some(mean)
}

/// Arithmetic mean over finite observations only.
pub(crate) fn mean_ignoring_nonfinite(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0_usize;
    for value in values.into_iter().filter(|value| value.is_finite()) {
        sum += value;
        count += 1;
    }
    let mean = (count > 0).then_some(sum / count as f64)?;
    mean.is_finite().then_some(mean)
}

/// Population variance with denominator `n`; rejects empty or non-finite input.
pub(crate) fn population_variance(values: &[f64]) -> Option<f64> {
    variance_with_denominator(values, values.len())
}

/// Sample variance with denominator `n - 1`; requires at least two observations.
pub(crate) fn sample_variance(values: &[f64]) -> Option<f64> {
    let denominator = values.len().checked_sub(1)?;
    if denominator == 0 {
        return None;
    }
    variance_with_denominator(values, denominator)
}

/// Sample standard deviation with denominator `n - 1`.
pub(crate) fn sample_standard_deviation(values: &[f64]) -> Option<f64> {
    let standard_deviation = sample_variance(values)?.sqrt();
    standard_deviation.is_finite().then_some(standard_deviation)
}

fn variance_with_denominator(values: &[f64], denominator: usize) -> Option<f64> {
    if denominator == 0 {
        return None;
    }
    let mean = mean_all_finite(values.iter().copied())?;
    let sum_squared_deviations = values.iter().try_fold(0.0, |sum, value| {
        let delta = *value - mean;
        let next = sum + delta * delta;
        next.is_finite().then_some(next)
    })?;
    let variance = sum_squared_deviations / denominator as f64;
    variance.is_finite().then_some(variance)
}

/// Minimum and maximum after dropping non-finite observations.
pub(crate) fn min_max_ignoring_nonfinite(values: &[f64]) -> Option<(f64, f64)> {
    let mut finite = values.iter().copied().filter(|value| value.is_finite());
    let first = finite.next()?;
    Some(finite.fold((first, first), |(min, max), value| {
        (min.min(value), max.max(value))
    }))
}

/// Nearest-rank percentile for finite ascending values.
///
/// Percentiles are accepted on the closed interval `[0, 1]`. Zero selects
/// the first value; positive percentiles use `ceil(p * n)`.
pub(crate) fn percentile_nearest_rank_sorted(
    sorted_values: &[f64],
    percentile: f64,
) -> Option<f64> {
    if !percentile.is_finite() || !(0.0..=1.0).contains(&percentile) {
        return None;
    }
    median_sorted_average_even(sorted_values)?;
    let rank = (percentile * sorted_values.len() as f64).ceil() as usize;
    sorted_values
        .get(rank.saturating_sub(1).min(sorted_values.len() - 1))
        .copied()
}

/// Finite division. Zero or non-finite denominators and non-finite results are undefined.
pub(crate) fn safe_finite_ratio(numerator: f64, denominator: f64) -> Option<f64> {
    if !numerator.is_finite() || !denominator.is_finite() || denominator == 0.0 {
        return None;
    }
    let ratio = numerator / denominator;
    ratio.is_finite().then_some(ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_uses_average_of_two_middle_values_for_even_samples() {
        assert_eq!(median_sorted_average_even(&[1.0, 2.0, 3.0]), Some(2.0));
        assert_eq!(
            median_sorted_average_even(&[1.0, 2.0, 8.0, 10.0]),
            Some(5.0)
        );
        assert_eq!(median_sorted_average_even(&[]), None);
        assert_eq!(median_sorted_average_even(&[2.0, 1.0]), None);
        assert_eq!(median_sorted_average_even(&[1.0, f64::NAN]), None);
    }

    #[test]
    fn median_sorting_and_nonfinite_policy_are_explicit() {
        let mut values = [8.0, 1.0, 10.0, 2.0];
        assert_eq!(median_average_even(&mut values), Some(5.0));

        let values = [1.0, f64::NAN, 3.0, f64::INFINITY];
        assert_eq!(median_ignoring_nonfinite(&values), Some(2.0));
    }

    #[test]
    fn mean_policies_distinguish_rejection_from_ignoring() {
        assert_eq!(mean_all_finite([1.0, 2.0, 3.0]), Some(2.0));
        assert_eq!(mean_all_finite([1.0, f64::NAN]), None);
        assert_eq!(mean_all_finite([]), None);

        assert_eq!(
            mean_ignoring_nonfinite([1.0, f64::NAN, 3.0, f64::INFINITY]),
            Some(2.0)
        );
        assert_eq!(mean_ignoring_nonfinite([f64::NAN]), None);
    }

    #[test]
    fn population_and_sample_variance_have_distinct_denominators() {
        let values = [1.0, 2.0, 3.0, 4.0];
        assert!((population_variance(&values).expect("population") - 1.25).abs() < 1.0e-12);
        assert!((sample_variance(&values).expect("sample") - (5.0 / 3.0)).abs() < 1.0e-12);
        assert!(
            (sample_standard_deviation(&values).expect("sample sd") - (5.0_f64 / 3.0).sqrt()).abs()
                < 1.0e-12
        );
        assert_eq!(population_variance(&[]), None);
        assert_eq!(sample_variance(&[1.0]), None);
        assert_eq!(sample_variance(&[1.0, f64::NAN]), None);
    }

    #[test]
    fn finite_extrema_percentile_and_ratio_contracts_are_explicit() {
        assert_eq!(
            min_max_ignoring_nonfinite(&[f64::NAN, 3.0, -2.0, f64::INFINITY]),
            Some((-2.0, 3.0))
        );
        assert_eq!(min_max_ignoring_nonfinite(&[f64::NAN]), None);

        let sorted = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile_nearest_rank_sorted(&sorted, 0.0), Some(1.0));
        assert_eq!(percentile_nearest_rank_sorted(&sorted, 0.5), Some(2.0));
        assert_eq!(percentile_nearest_rank_sorted(&sorted, 0.95), Some(4.0));
        assert_eq!(percentile_nearest_rank_sorted(&sorted, f64::NAN), None);
        assert_eq!(percentile_nearest_rank_sorted(&[2.0, 1.0], 0.5), None);

        assert_eq!(safe_finite_ratio(6.0, 3.0), Some(2.0));
        assert_eq!(safe_finite_ratio(0.0, 0.0), None);
        assert_eq!(safe_finite_ratio(1.0, f64::INFINITY), None);
        assert_eq!(safe_finite_ratio(f64::MAX, f64::MIN_POSITIVE), None);
    }
}

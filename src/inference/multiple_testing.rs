use crate::errors::{MarklabError, Result};

/// Benjamini-Hochberg adjusted p-values in the caller's original order.
///
/// Every input must be finite and lie in `[0, 1]`. Inclusive ties are ordered
/// deterministically by original index; the reverse cumulative minimum makes
/// the adjusted values monotone in sorted p-value order.
pub(crate) fn benjamini_hochberg(p_values: &[f64]) -> Result<Vec<f64>> {
    for (index, p_value) in p_values.iter().copied().enumerate() {
        if !p_value.is_finite() || !(0.0..=1.0).contains(&p_value) {
            return Err(MarklabError::Validation(format!(
                "Benjamini-Hochberg p-value {index} must be finite and between zero and one"
            )));
        }
    }

    let mut indexed = p_values.iter().copied().enumerate().collect::<Vec<_>>();
    indexed.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });

    let count = indexed.len();
    let mut adjusted = vec![0.0; count];
    let mut next = 1.0_f64;
    for (zero_based_rank, (original_index, p_value)) in indexed.into_iter().enumerate().rev() {
        let rank = zero_based_rank + 1;
        let q_value = (p_value * count as f64 / rank as f64).min(next).min(1.0);
        adjusted[original_index] = q_value;
        next = q_value;
    }
    Ok(adjusted)
}

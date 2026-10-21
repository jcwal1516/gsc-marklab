use std::cmp::Ordering;

use crate::NumericsError;

/// Global extreme-rank-length envelope and p-value for one observed curve.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtremeRankLengthEnvelope {
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
    pub p_global: f64,
    pub observed_depth: f64,
    pub critical_depth: f64,
}

pub fn extreme_rank_length_envelope(
    observed: &[f64],
    permutations: &[Vec<f64>],
    alpha: f64,
) -> Result<ExtremeRankLengthEnvelope, NumericsError> {
    if observed.is_empty()
        || permutations.is_empty()
        || !alpha.is_finite()
        || alpha <= 0.0
        || alpha >= 1.0
        || observed.iter().any(|value| !value.is_finite())
        || permutations.iter().any(|curve| {
            curve.len() != observed.len() || curve.iter().any(|value| !value.is_finite())
        })
    {
        return Err(NumericsError::Invalid(
            "ERL requires finite nonempty equal-length curves and alpha in (0,1)".into(),
        ));
    }
    let mut curves = Vec::with_capacity(permutations.len() + 1);
    curves.push(observed.to_vec());
    curves.extend_from_slice(permutations);
    let curve_count = curves.len();
    let mut rank_vectors = vec![vec![0.0; observed.len()]; curve_count];
    for point in 0..observed.len() {
        let values = curves.iter().map(|curve| curve[point]).collect::<Vec<_>>();
        let ranks = average_ranks(&values);
        for (curve, rank) in ranks.into_iter().enumerate() {
            rank_vectors[curve][point] = rank.min(curve_count as f64 + 1.0 - rank);
        }
    }
    for ranks in &mut rank_vectors {
        ranks.sort_by(f64::total_cmp);
    }
    let depths = normalized_depths(&rank_vectors);
    let observed_depth = depths[0];
    let p_global = (1 + depths
        .iter()
        .skip(1)
        .filter(|depth| **depth <= observed_depth)
        .count()) as f64
        / curve_count as f64;
    let mut ascending = depths.clone();
    ascending.sort_by(f64::total_cmp);
    // Exclude at most floor(alpha * n) curves, retaining cutoff ties.
    let mut critical_index = ((alpha * curve_count as f64).floor() as usize).min(curve_count - 1);
    // Keep the cutoff consistent with p-values at floating-point grid boundaries.
    while critical_index > 0 && critical_index as f64 / curve_count as f64 > alpha {
        critical_index -= 1;
    }
    while critical_index + 1 < curve_count
        && (critical_index + 1) as f64 / curve_count as f64 <= alpha
    {
        critical_index += 1;
    }
    let critical_depth = ascending[critical_index];
    let mut lower = vec![f64::INFINITY; observed.len()];
    let mut upper = vec![f64::NEG_INFINITY; observed.len()];
    for (curve, depth) in curves.iter().zip(depths) {
        if depth < critical_depth {
            continue;
        }
        for (point, value) in curve.iter().enumerate() {
            lower[point] = lower[point].min(*value);
            upper[point] = upper[point].max(*value);
        }
    }
    Ok(ExtremeRankLengthEnvelope {
        lower,
        upper,
        p_global,
        observed_depth,
        critical_depth,
    })
}

fn average_ranks(values: &[f64]) -> Vec<f64> {
    let mut order = (0..values.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| values[*left].total_cmp(&values[*right]));
    let mut ranks = vec![0.0; values.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && values[order[end]] == values[order[start]] {
            end += 1;
        }
        let average = (start + 1 + end) as f64 / 2.0;
        for index in &order[start..end] {
            ranks[*index] = average;
        }
        start = end;
    }
    ranks
}

fn normalized_depths(rank_vectors: &[Vec<f64>]) -> Vec<f64> {
    let mut order = (0..rank_vectors.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| lexicographic_cmp(&rank_vectors[*left], &rank_vectors[*right]));
    let mut depths = vec![0.0; rank_vectors.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && rank_vectors[order[end]] == rank_vectors[order[start]] {
            end += 1;
        }
        let depth = (start + 1 + end) as f64 / 2.0 / rank_vectors.len() as f64;
        for index in &order[start..end] {
            depths[*index] = depth;
        }
        start = end;
    }
    depths
}

fn lexicographic_cmp(left: &[f64], right: &[f64]) -> Ordering {
    left.iter()
        .zip(right)
        .find_map(|(left, right)| {
            let ordering = left.total_cmp(right);
            (ordering != Ordering::Equal).then_some(ordering)
        })
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_curves_match_the_canonical_erl_tie_oracle() {
        let observed = [1.0, 2.0, 3.0];
        let permutations = vec![observed.to_vec(), observed.to_vec(), observed.to_vec()];
        let envelope = extreme_rank_length_envelope(&observed, &permutations, 0.25).unwrap();
        assert_eq!(envelope.lower, observed);
        assert_eq!(envelope.upper, observed);
        assert_eq!(envelope.p_global, 1.0);
        assert_eq!(envelope.observed_depth, 0.625);
        assert_eq!(envelope.critical_depth, 0.625);
    }
}

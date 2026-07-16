#[derive(Clone, Debug, PartialEq)]
pub struct GlobalEnvelope {
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
    pub p_global: f64,
    pub erl_depth: f64,
    pub critical_depth: f64,
    pub n_permutations: usize,
}

use crate::errors::{MmrspaceError, Result};

impl GlobalEnvelope {
    pub fn from_curves(observed: &[f64], permutations: &[Vec<f64>], alpha: f64) -> Result<Self> {
        Self::from_curves_with_eligibility(
            observed,
            permutations,
            alpha,
            &vec![true; observed.len()],
        )
    }

    pub fn from_curves_with_eligibility(
        observed: &[f64],
        permutations: &[Vec<f64>],
        alpha: f64,
        inference_eligible: &[bool],
    ) -> Result<Self> {
        if observed.is_empty() {
            return Err(MmrspaceError::Validation(
                "observed curve must not be empty".into(),
            ));
        }
        if permutations.is_empty() {
            return Err(MmrspaceError::Validation(
                "global envelope requires at least one permutation curve".into(),
            ));
        }
        if permutations
            .iter()
            .any(|curve| curve.len() != observed.len())
        {
            return Err(MmrspaceError::Validation(
                "all permutation curves must match observed curve length".into(),
            ));
        }
        if inference_eligible.len() != observed.len()
            || !inference_eligible.iter().any(|value| *value)
        {
            return Err(MmrspaceError::Validation(
                "global envelope requires at least one inference-eligible curve point".into(),
            ));
        }
        if !(alpha.is_finite() && 0.0 < alpha && alpha < 1.0) {
            return Err(MmrspaceError::Validation(
                "global-envelope alpha must be finite and strictly between zero and one".into(),
            ));
        }
        let n_curves = permutations.len() + 1;
        if n_curves as f64 * alpha < 1.0 {
            return Err(MmrspaceError::Validation(format!(
                "global envelope requires (B + 1) * alpha >= 1 (got {n_curves} curves and alpha {alpha})"
            )));
        }

        let mut curves = Vec::with_capacity(n_curves);
        curves.push(observed.to_vec());
        curves.extend_from_slice(permutations);
        if curves.iter().flatten().any(|value| !value.is_finite()) {
            return Err(MmrspaceError::Compute(
                "global-envelope curves contain a non-finite value".into(),
            ));
        }

        let inference_curves = curves
            .iter()
            .map(|curve| {
                curve
                    .iter()
                    .zip(inference_eligible)
                    .filter_map(|(value, eligible)| eligible.then_some(*value))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let rank_vectors = extreme_rank_length_vectors(&inference_curves);
        let depths = normalized_erl_depths(&rank_vectors);
        let erl_depth = *depths
            .first()
            .ok_or_else(|| MmrspaceError::Compute("missing observed ERL depth".into()))?;
        let outside_count = depths
            .iter()
            .skip(1)
            .filter(|depth| **depth <= erl_depth)
            .count();
        let p_global = (outside_count + 1) as f64 / n_curves as f64;

        let mut descending_depths = depths.clone();
        descending_depths.sort_by(|left, right| right.total_cmp(left));
        let critical_index = ((1.0 - alpha) * n_curves as f64).floor() as usize;
        let critical_depth = descending_depths[critical_index.saturating_sub(1)];

        let mut lower = vec![f64::INFINITY; observed.len()];
        let mut upper = vec![f64::NEG_INFINITY; observed.len()];
        for (curve, depth) in curves.iter().zip(depths.iter()) {
            if *depth < critical_depth {
                continue;
            }
            for (index, value) in curve.iter().copied().enumerate() {
                lower[index] = lower[index].min(value);
                upper[index] = upper[index].max(value);
            }
        }

        Ok(Self {
            lower,
            upper,
            p_global,
            erl_depth,
            critical_depth,
            n_permutations: permutations.len(),
        })
    }
}

fn extreme_rank_length_vectors(curves: &[Vec<f64>]) -> Vec<Vec<f64>> {
    if curves.is_empty() {
        return Vec::new();
    }
    let n_points = curves[0].len();
    let mut rank_vectors = vec![Vec::with_capacity(n_points); curves.len()];

    for point_index in 0..n_points {
        let values = curves
            .iter()
            .map(|curve| curve[point_index])
            .collect::<Vec<_>>();
        let pointwise_ranks = average_ranks(&values);
        for (curve_index, rank) in pointwise_ranks.into_iter().enumerate() {
            rank_vectors[curve_index].push(rank.min(curves.len() as f64 + 1.0 - rank));
        }
    }

    for rank_vector in &mut rank_vectors {
        rank_vector.sort_by(f64::total_cmp);
    }
    rank_vectors
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
        let average_rank = (start + 1 + end) as f64 / 2.0;
        for index in &order[start..end] {
            ranks[*index] = average_rank;
        }
        start = end;
    }
    ranks
}

fn normalized_erl_depths(rank_vectors: &[Vec<f64>]) -> Vec<f64> {
    let mut order = (0..rank_vectors.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| lexicographic_cmp(&rank_vectors[*left], &rank_vectors[*right]));

    let mut depths = vec![0.0; rank_vectors.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && rank_vectors[order[end]] == rank_vectors[order[start]] {
            end += 1;
        }
        let average_rank = (start + 1 + end) as f64 / 2.0;
        let depth = average_rank / rank_vectors.len() as f64;
        for index in &order[start..end] {
            depths[*index] = depth;
        }
        start = end;
    }
    depths
}

fn lexicographic_cmp(left: &[f64], right: &[f64]) -> std::cmp::Ordering {
    left.iter()
        .zip(right.iter())
        .find_map(|(left_value, right_value)| {
            let ordering = left_value.total_cmp(right_value);
            (ordering != std::cmp::Ordering::Equal).then_some(ordering)
        })
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

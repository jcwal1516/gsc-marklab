#[derive(Clone, Debug, PartialEq)]
pub struct GlobalEnvelope {
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
    pub p_global: f64,
    pub erl_depth: f64,
    pub critical_depth: f64,
    pub n_permutations: usize,
}

use crate::{
    common::matrix::F64Matrix,
    errors::{MarklabError, Result},
};

impl GlobalEnvelope {
    #[cfg(test)]
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
            return Err(MarklabError::Validation(
                "observed curve must not be empty".into(),
            ));
        }
        if permutations.is_empty() {
            return Err(MarklabError::Validation(
                "global envelope requires at least one permutation curve".into(),
            ));
        }
        if permutations
            .iter()
            .any(|curve| curve.len() != observed.len())
        {
            return Err(MarklabError::Validation(
                "all permutation curves must match observed curve length".into(),
            ));
        }
        let matrix = F64Matrix::from_rows(permutations).ok_or_else(|| {
            MarklabError::Validation(
                "all permutation curves must match observed curve length".into(),
            )
        })?;
        Self::from_matrix_with_eligibility(observed, &matrix, alpha, inference_eligible)
    }

    pub(crate) fn from_matrix_with_eligibility(
        observed: &[f64],
        permutations: &F64Matrix,
        alpha: f64,
        inference_eligible: &[bool],
    ) -> Result<Self> {
        if observed.is_empty() {
            return Err(MarklabError::Validation(
                "observed curve must not be empty".into(),
            ));
        }
        if permutations.row_count() == 0 {
            return Err(MarklabError::Validation(
                "global envelope requires at least one permutation curve".into(),
            ));
        }
        if permutations.column_count() != observed.len() {
            return Err(MarklabError::Validation(
                "all permutation curves must match observed curve length".into(),
            ));
        }
        if inference_eligible.len() != observed.len()
            || !inference_eligible.iter().any(|value| *value)
        {
            return Err(MarklabError::Validation(
                "global envelope requires at least one inference-eligible curve point".into(),
            ));
        }
        if !(alpha.is_finite() && 0.0 < alpha && alpha < 1.0) {
            return Err(MarklabError::Validation(
                "global-envelope alpha must be finite and strictly between zero and one".into(),
            ));
        }
        let n_curves = permutations.row_count() + 1;
        if n_curves as f64 * alpha < 1.0 {
            return Err(MarklabError::Validation(format!(
                "global envelope requires (B + 1) * alpha >= 1 (got {n_curves} curves and alpha {alpha})"
            )));
        }

        if observed.iter().any(|value| !value.is_finite())
            || permutations.values().iter().any(|value| !value.is_finite())
        {
            return Err(MarklabError::Compute(
                "global-envelope curves contain a non-finite value".into(),
            ));
        }

        let eligible_positions = inference_eligible
            .iter()
            .enumerate()
            .filter_map(|(index, eligible)| eligible.then_some(index))
            .collect::<Vec<_>>();
        let mut inference_curves = F64Matrix::zeros(n_curves, eligible_positions.len())
            .ok_or_else(|| MarklabError::Compute("invalid ERL matrix dimensions".into()))?;
        for (column, source) in eligible_positions.iter().copied().enumerate() {
            inference_curves.row_mut(0).expect("observed ERL row")[column] = observed[source];
            for permutation_index in 0..permutations.row_count() {
                inference_curves
                    .row_mut(permutation_index + 1)
                    .expect("permutation ERL row")[column] = permutations
                    .row(permutation_index)
                    .expect("permutation row")[source];
            }
        }
        let rank_vectors = extreme_rank_length_vectors(&inference_curves);
        let depths = normalized_erl_depths(&rank_vectors);
        let erl_depth = *depths
            .first()
            .ok_or_else(|| MarklabError::Compute("missing observed ERL depth".into()))?;
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
        for (curve_index, depth) in depths.iter().copied().enumerate() {
            if depth < critical_depth {
                continue;
            }
            let curve = if curve_index == 0 {
                observed
            } else {
                permutations
                    .row(curve_index - 1)
                    .expect("validated permutation row")
            };
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
            n_permutations: permutations.row_count(),
        })
    }
}

fn extreme_rank_length_vectors(curves: &F64Matrix) -> F64Matrix {
    let mut rank_vectors =
        F64Matrix::zeros(curves.row_count(), curves.column_count()).expect("nonempty ERL matrix");

    for point_index in 0..curves.column_count() {
        let values = curves
            .iter_rows()
            .map(|curve| curve[point_index])
            .collect::<Vec<_>>();
        let pointwise_ranks = average_ranks(&values);
        for (curve_index, rank) in pointwise_ranks.into_iter().enumerate() {
            rank_vectors.row_mut(curve_index).expect("rank row")[point_index] =
                rank.min(curves.row_count() as f64 + 1.0 - rank);
        }
    }

    for rank_vector in rank_vectors.iter_rows_mut() {
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

fn normalized_erl_depths(rank_vectors: &F64Matrix) -> Vec<f64> {
    let mut order = (0..rank_vectors.row_count()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        lexicographic_cmp(
            rank_vectors.row(*left).expect("rank row"),
            rank_vectors.row(*right).expect("rank row"),
        )
    });

    let mut depths = vec![0.0; rank_vectors.row_count()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len()
            && rank_vectors.row(order[end]).expect("rank row")
                == rank_vectors.row(order[start]).expect("rank row")
        {
            end += 1;
        }
        let average_rank = (start + 1 + end) as f64 / 2.0;
        let depth = average_rank / rank_vectors.row_count() as f64;
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

#![forbid(unsafe_code)]
//! Stable bounded numerical primitives for Marklab.

mod bfgs;
pub use bfgs::minimize_bfgs;

/// Compile-embedded source of the bounded optimizer, for source-bound scientific provenance.
/// This is opaque identity material, not a source-parsing or algorithm configuration interface.
pub const BFGS_IMPLEMENTATION_SOURCE: &str = include_str!("bfgs.rs");

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAXIMUM_ELEMENTS: u64 = 100_000_000;
const MAXIMUM_COVARIANCE_CROSS_PRODUCTS: u64 = 250_000_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StablePrimitivesSpec {
    pub log_values: Vec<f64>,
    pub weighted_values: Vec<f64>,
    pub weights: Vec<f64>,
    pub matrix: Vec<Vec<f64>>,
    pub covariance_weights: Option<Vec<f64>>,
    pub maximum_elements: u64,
    pub maximum_covariance_cross_products: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct StablePrimitivesResult {
    pub format: &'static str,
    pub version: u32,
    pub log_sum_exp_algorithm: &'static str,
    pub log_sum_exp: f64,
    pub log_mean_exp: f64,
    pub weighted_mean_algorithm: &'static str,
    pub weighted_mean: f64,
    pub covariance_algorithm: &'static str,
    pub covariance_weighting: &'static str,
    pub covariance_mean: Vec<f64>,
    pub covariance: Vec<Vec<f64>>,
    pub covariance_effective_sample_size: f64,
    pub input_elements: u64,
    pub covariance_cross_products: u64,
    pub claim_status: &'static str,
}

#[derive(Debug, Error)]
pub enum NumericsError {
    #[error("invalid numerical primitive input: {0}")]
    Invalid(String),
    #[error("numerical primitive resource limit exceeded: {0}")]
    Resource(String),
    #[error("numerical primitive failure: {0}")]
    Numerical(String),
}

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
    let mut descending = depths.clone();
    descending.sort_by(|left, right| right.total_cmp(left));
    let critical_index = ((1.0 - alpha) * curve_count as f64).floor() as usize;
    let critical_depth = descending[critical_index.saturating_sub(1)];
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

pub fn evaluate_stable_primitives(
    spec: StablePrimitivesSpec,
) -> Result<StablePrimitivesResult, NumericsError> {
    let (elements, cross_products, columns) = validate(&spec)?;
    let maximum = spec
        .log_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let exponential_sum = neumaier_sum(spec.log_values.iter().map(|value| (value - maximum).exp()));
    let log_sum_exp = maximum + exponential_sum.ln();
    let log_mean_exp = log_sum_exp - (spec.log_values.len() as f64).ln();

    let weight_sum = neumaier_sum(spec.weights.iter().copied());
    let normalized_weights = spec
        .weights
        .iter()
        .map(|weight| weight / weight_sum)
        .collect::<Vec<_>>();
    let weighted_mean = neumaier_sum(
        spec.weighted_values
            .iter()
            .zip(&normalized_weights)
            .map(|(value, weight)| value * weight),
    );

    let covariance_weights = match &spec.covariance_weights {
        Some(weights) => {
            let sum = neumaier_sum(weights.iter().copied());
            weights
                .iter()
                .map(|weight| weight / sum)
                .collect::<Vec<_>>()
        }
        None => vec![1.0 / spec.matrix.len() as f64; spec.matrix.len()],
    };
    let covariance_mean = (0..columns)
        .map(|column| {
            neumaier_sum(
                spec.matrix
                    .iter()
                    .zip(&covariance_weights)
                    .map(|(row, weight)| row[column] * weight),
            )
        })
        .collect::<Vec<_>>();
    let squared_weight_sum = neumaier_sum(covariance_weights.iter().map(|weight| weight * weight));
    let denominator = 1.0 - squared_weight_sum;
    let effective_sample_size = 1.0 / squared_weight_sum;
    if !denominator.is_finite() || denominator <= 0.0 || !effective_sample_size.is_finite() {
        return Err(NumericsError::Invalid(
            "covariance weights have insufficient effective sample size".into(),
        ));
    }
    let mut covariance = vec![vec![0.0; columns]; columns];
    for left in 0..columns {
        for right in left..columns {
            let cross = neumaier_sum(spec.matrix.iter().zip(&covariance_weights).map(
                |(row, weight)| {
                    weight
                        * (row[left] - covariance_mean[left])
                        * (row[right] - covariance_mean[right])
                },
            )) / denominator;
            covariance[left][right] = cross;
            covariance[right][left] = cross;
        }
    }
    if [
        log_sum_exp,
        log_mean_exp,
        weighted_mean,
        effective_sample_size,
    ]
    .iter()
    .any(|value| !value.is_finite())
        || covariance_mean.iter().any(|value| !value.is_finite())
        || covariance.iter().flatten().any(|value| !value.is_finite())
    {
        return Err(NumericsError::Numerical(
            "a stable primitive result is not finite".into(),
        ));
    }
    Ok(StablePrimitivesResult {
        format: "marklab.stable_numerical_primitives",
        version: 1,
        log_sum_exp_algorithm: "max_shift_neumaier_f64",
        log_sum_exp,
        log_mean_exp,
        weighted_mean_algorithm: "normalized_weight_neumaier_f64",
        weighted_mean,
        covariance_algorithm: "two_pass_neumaier_symmetric_f64",
        covariance_weighting: if spec.covariance_weights.is_some() {
            "caller_weighted_unbiased_effective_denominator"
        } else {
            "unweighted_sample_n_minus_one"
        },
        covariance_mean,
        covariance,
        covariance_effective_sample_size: effective_sample_size,
        input_elements: elements,
        covariance_cross_products: cross_products,
        claim_status: "deterministic_numerical_primitive_only",
    })
}

fn validate(spec: &StablePrimitivesSpec) -> Result<(u64, u64, usize), NumericsError> {
    if spec.log_values.is_empty()
        || spec.log_values.iter().any(|value| !value.is_finite())
        || spec.weighted_values.is_empty()
        || spec.weighted_values.len() != spec.weights.len()
        || spec.weighted_values.iter().any(|value| !value.is_finite())
        || spec
            .weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight < 0.0)
        || neumaier_sum(spec.weights.iter().copied()) <= 0.0
    {
        return Err(NumericsError::Invalid(
            "log/weighted inputs violate finite nonempty aligned positive-mass requirements".into(),
        ));
    }
    if spec.matrix.len() < 2 || spec.matrix.first().is_none_or(Vec::is_empty) {
        return Err(NumericsError::Invalid(
            "covariance matrix requires at least two nonempty rows".into(),
        ));
    }
    let columns = spec.matrix[0].len();
    if spec
        .matrix
        .iter()
        .any(|row| row.len() != columns || row.iter().any(|value| !value.is_finite()))
    {
        return Err(NumericsError::Invalid(
            "covariance matrix must be rectangular and finite".into(),
        ));
    }
    if let Some(weights) = &spec.covariance_weights {
        if weights.len() != spec.matrix.len()
            || weights
                .iter()
                .any(|weight| !weight.is_finite() || *weight < 0.0)
            || neumaier_sum(weights.iter().copied()) <= 0.0
        {
            return Err(NumericsError::Invalid(
                "covariance weights violate aligned nonnegative positive-mass requirements".into(),
            ));
        }
    }
    let matrix_elements = u64::try_from(spec.matrix.len())
        .ok()
        .and_then(|rows| {
            u64::try_from(columns)
                .ok()
                .and_then(|cols| rows.checked_mul(cols))
        })
        .ok_or_else(|| NumericsError::Resource("matrix element count overflowed".into()))?;
    let elements = matrix_elements
        .checked_add(spec.log_values.len() as u64)
        .and_then(|value| value.checked_add(spec.weighted_values.len() as u64))
        .and_then(|value| value.checked_add(spec.weights.len() as u64))
        .and_then(|value| {
            value.checked_add(
                spec.covariance_weights
                    .as_ref()
                    .map_or(0, |weights| weights.len() as u64),
            )
        })
        .ok_or_else(|| NumericsError::Resource("input element count overflowed".into()))?;
    if elements > spec.maximum_elements || elements > MAXIMUM_ELEMENTS {
        return Err(NumericsError::Resource(format!(
            "input elements {elements} exceed caller or built-in maximum"
        )));
    }
    let triangle = columns
        .checked_mul(columns + 1)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| NumericsError::Resource("covariance triangle overflowed".into()))?;
    let cross_products = u64::try_from(spec.matrix.len())
        .ok()
        .and_then(|rows| {
            u64::try_from(triangle)
                .ok()
                .and_then(|pairs| rows.checked_mul(pairs))
        })
        .ok_or_else(|| NumericsError::Resource("covariance work overflowed".into()))?;
    if cross_products > spec.maximum_covariance_cross_products
        || cross_products > MAXIMUM_COVARIANCE_CROSS_PRODUCTS
    {
        return Err(NumericsError::Resource(format!(
            "covariance cross products {cross_products} exceed caller or built-in maximum"
        )));
    }
    Ok((elements, cross_products, columns))
}

fn neumaier_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        let next = sum + value;
        if sum.abs() >= value.abs() {
            correction += (sum - next) + value;
        } else {
            correction += (value - next) + sum;
        }
        sum = next;
    }
    sum + correction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covariance_rejects_one_effective_observation() {
        let error = evaluate_stable_primitives(StablePrimitivesSpec {
            log_values: vec![0.0],
            weighted_values: vec![1.0],
            weights: vec![1.0],
            matrix: vec![vec![1.0], vec![2.0]],
            covariance_weights: Some(vec![1.0, 0.0]),
            maximum_elements: 10,
            maximum_covariance_cross_products: 10,
        })
        .unwrap_err();
        assert!(error.to_string().contains("effective sample size"));
    }

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

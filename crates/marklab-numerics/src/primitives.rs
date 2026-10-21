use serde::{Deserialize, Serialize};

use crate::{summation::neumaier_sum, NumericsError};

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

    let normalized_weights = normalize_weights(&spec.weights);
    let weighted_mean = neumaier_sum(
        spec.weighted_values
            .iter()
            .zip(&normalized_weights)
            .map(|(value, weight)| value * weight),
    );

    let covariance_weights = match &spec.covariance_weights {
        Some(weights) => normalize_weights(weights),
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
    let (denominator, effective_sample_size) = if spec.covariance_weights.is_some() {
        let squared_weight_sum =
            neumaier_sum(covariance_weights.iter().map(|weight| weight * weight));
        (1.0 - squared_weight_sum, 1.0 / squared_weight_sum)
    } else {
        // Realize the n - 1 sample correction directly, while keeping the
        // normalized cross products that avoid overflowing an unscaled sum.
        let rows = spec.matrix.len();
        ((rows - 1) as f64 / rows as f64, rows as f64)
    };
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
        || !spec.weights.iter().any(|weight| *weight > 0.0)
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
            || !weights.iter().any(|weight| *weight > 0.0)
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

fn normalize_weights(weights: &[f64]) -> Vec<f64> {
    // Common rescaling leaves the estimand unchanged and avoids overflowing the weight sum.
    let maximum = weights.iter().copied().fold(0.0, f64::max);
    let sum = neumaier_sum(weights.iter().map(|weight| weight / maximum));
    weights
        .iter()
        .map(|weight| (weight / maximum) / sum)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_moments_are_invariant_to_finite_weight_scale() {
        for weight in [1.0, f64::MAX, f64::from_bits(1)] {
            let result = evaluate_stable_primitives(StablePrimitivesSpec {
                log_values: vec![0.0],
                weighted_values: vec![1.0, 3.0],
                weights: vec![weight, weight],
                matrix: vec![vec![1.0], vec![3.0]],
                covariance_weights: Some(vec![weight, weight]),
                maximum_elements: 10,
                maximum_covariance_cross_products: 10,
            })
            .expect("rescaling equal finite weights preserves the same moments");
            assert_eq!(result.weighted_mean, 2.0);
            assert_eq!(result.covariance_mean, [2.0]);
            assert_eq!(result.covariance, [vec![2.0]]);
            assert_eq!(result.covariance_effective_sample_size, 2.0);
        }
    }

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
}

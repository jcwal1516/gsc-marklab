use crate::{linalg, BayesError};

use super::DistanceToResourceSpec;

pub(super) fn posterior(
    spec: &DistanceToResourceSpec,
    design: &[f64],
    dimension: usize,
    fixed: usize,
) -> Result<(Vec<f64>, Vec<f64>), BayesError> {
    let inverse_noise_variance = spec.known_noise_sd.powi(-2);
    let mut precision = vec![0.0; dimension * dimension];
    let mut right = vec![0.0; dimension];
    for index in 0..dimension {
        let prior_sd = if index < fixed {
            spec.coefficient_prior_sd
        } else {
            spec.patient_effect_prior_sd
        };
        precision[index * dimension + index] = prior_sd.powi(-2);
    }
    for (row_index, row) in design.chunks_exact(dimension).enumerate() {
        let outcome = spec.observations[row_index].outcome;
        for i in 0..dimension {
            right[i] += row[i] * outcome * inverse_noise_variance;
            for j in 0..dimension {
                precision[i * dimension + j] += row[i] * row[j] * inverse_noise_variance;
            }
        }
    }
    let cholesky = linalg::cholesky(&precision, dimension).ok_or_else(|| {
        BayesError::InvalidSpec("distance posterior precision is not positive definite".into())
    })?;
    let mean = solve_cholesky(&cholesky, &right, dimension);
    let mut covariance = vec![0.0; dimension * dimension];
    for column in 0..dimension {
        let mut unit = vec![0.0; dimension];
        unit[column] = 1.0;
        let solution = solve_cholesky(&cholesky, &unit, dimension);
        for row in 0..dimension {
            covariance[row * dimension + column] = solution[row];
        }
    }
    Ok((mean, covariance))
}

fn solve_cholesky(lower: &[f64], right: &[f64], dimension: usize) -> Vec<f64> {
    let intermediate = linalg::solve_lower_dot(lower, dimension, right);
    let mut result = vec![0.0; dimension];
    for row in (0..dimension).rev() {
        let prior = (row + 1..dimension)
            .map(|column| lower[column * dimension + row] * result[column])
            .sum::<f64>();
        result[row] = (intermediate[row] - prior) / lower[row * dimension + row];
    }
    result
}

pub(super) fn linear_summary(values: &[f64], mean: &[f64], covariance: &[f64]) -> (f64, f64) {
    let dimension = mean.len();
    let location = values.iter().zip(mean).map(|(x, beta)| x * beta).sum();
    let variance = (0..dimension)
        .flat_map(|i| {
            (0..dimension).map(move |j| values[i] * covariance[i * dimension + j] * values[j])
        })
        .sum::<f64>()
        .max(0.0);
    (location, variance.sqrt())
}

use std::collections::BTreeSet;

use serde::Serialize;
use thiserror::Error;

use crate::{DiagonalPolicy, NormalizationPolicy, SymmetryPolicy, ValidatedSpatialWeights};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SarModelType {
    Lag,
    Error,
}

#[derive(Clone, Debug)]
pub struct SarSpec {
    pub model_type: SarModelType,
    pub response: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
    pub intercept: f64,
    pub coefficients: Vec<f64>,
    pub rho: f64,
    pub sigma: f64,
    pub weights: ValidatedSpatialWeights,
}

#[derive(Clone, Debug)]
pub struct SarImpact {
    pub predictor: String,
    pub direct: f64,
    pub indirect: f64,
    pub total: f64,
}

#[derive(Clone, Debug)]
pub struct SarLikelihoodResult {
    pub log_likelihood: f64,
    pub log_abs_determinant: f64,
    pub residual_sum_squares: f64,
    pub impacts: Vec<SarImpact>,
}

#[derive(Debug, Error)]
pub enum SarError {
    #[error("invalid SAR input: {0}")]
    InvalidInput(String),
    #[error("SAR numerical failure: {0}")]
    Numerical(String),
}

pub fn sar_gaussian_log_likelihood(spec: SarSpec) -> Result<SarLikelihoodResult, SarError> {
    validate_spec(&spec)?;
    let dimension = spec.weights.region_ids.len();
    let predictors = spec.predictor_names.len();
    let mut system = vec![0.0; dimension * dimension];
    for index in 0..dimension {
        system[index * dimension + index] = 1.0;
    }
    for edge in &spec.weights.weights {
        system[edge.source_index * dimension + edge.target_index] -= spec.rho * edge.weight;
    }
    let factor = LuFactor::new(system, dimension)?;
    let fitted = (0..dimension)
        .map(|row| {
            spec.intercept
                + (0..predictors)
                    .map(|column| {
                        spec.design[row * predictors + column] * spec.coefficients[column]
                    })
                    .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let residual = match spec.model_type {
        SarModelType::Lag => multiply(&factor.original, &spec.response, dimension)
            .into_iter()
            .zip(fitted)
            .map(|(transformed_response, fitted)| transformed_response - fitted)
            .collect::<Vec<_>>(),
        SarModelType::Error => {
            let ordinary = spec
                .response
                .iter()
                .zip(fitted)
                .map(|(response, fitted)| response - fitted)
                .collect::<Vec<_>>();
            multiply(&factor.original, &ordinary, dimension)
        }
    };
    let residual_sum_squares = residual.iter().map(|value| value * value).sum::<f64>();
    let log_likelihood = factor.log_abs_determinant
        - dimension as f64 * spec.sigma.ln()
        - 0.5 * dimension as f64 * (2.0 * std::f64::consts::PI).ln()
        - 0.5 * residual_sum_squares / spec.sigma.powi(2);
    if !residual_sum_squares.is_finite() || !log_likelihood.is_finite() {
        return Err(SarError::Numerical(
            "likelihood calculation produced a non-finite value".into(),
        ));
    }
    let impacts = if spec.model_type == SarModelType::Lag {
        let inverse = factor.inverse()?;
        let direct_multiplier = (0..dimension)
            .map(|index| inverse[index * dimension + index])
            .sum::<f64>()
            / dimension as f64;
        let total_multiplier = inverse.iter().sum::<f64>() / dimension as f64;
        spec.predictor_names
            .into_iter()
            .zip(spec.coefficients)
            .map(|(predictor, coefficient)| {
                let direct = coefficient * direct_multiplier;
                let total = coefficient * total_multiplier;
                SarImpact {
                    predictor,
                    direct,
                    indirect: total - direct,
                    total,
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(SarLikelihoodResult {
        log_likelihood,
        log_abs_determinant: factor.log_abs_determinant,
        residual_sum_squares,
        impacts,
    })
}

fn validate_spec(spec: &SarSpec) -> Result<(), SarError> {
    let dimension = spec.weights.region_ids.len();
    let predictors = spec.predictor_names.len();
    if !(2..=512).contains(&dimension) || spec.weights.weights.len() > 200_000 {
        return Err(SarError::InvalidInput(
            "SAR requires 2-512 regions and at most 200000 edges".into(),
        ));
    }
    if spec.weights.policy.symmetry != SymmetryPolicy::NotRequired
        || spec.weights.policy.diagonal != DiagonalPolicy::Zero
        || spec.weights.policy.normalization != NormalizationPolicy::RowStandardize
        || !spec.weights.islands.is_empty()
    {
        return Err(SarError::InvalidInput(
            "SAR requires island-free row-standardized zero-diagonal weights without a symmetry requirement"
                .into(),
        ));
    }
    if !(1..=32).contains(&predictors)
        || spec.response.len() != dimension
        || spec.design.len() != dimension * predictors
        || spec.coefficients.len() != predictors
    {
        return Err(SarError::InvalidInput(
            "SAR response/design/coefficient dimensions are inconsistent".into(),
        ));
    }
    let mut names = BTreeSet::new();
    if spec.predictor_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || name == "intercept"
            || !names.insert(name.as_str())
    }) {
        return Err(SarError::InvalidInput(
            "SAR predictor names must be exact, unique, and not intercept".into(),
        ));
    }
    if !spec.intercept.is_finite()
        || !spec.rho.is_finite()
        || !spec.sigma.is_finite()
        || spec.sigma <= 0.0
        || spec.response.iter().any(|value| !value.is_finite())
        || spec.design.iter().any(|value| !value.is_finite())
        || spec.coefficients.iter().any(|value| !value.is_finite())
    {
        return Err(SarError::InvalidInput(
            "SAR parameters, response, design, and coefficients must be finite with positive sigma"
                .into(),
        ));
    }
    Ok(())
}

fn multiply(matrix: &[f64], vector: &[f64], dimension: usize) -> Vec<f64> {
    (0..dimension)
        .map(|row| {
            (0..dimension)
                .map(|column| matrix[row * dimension + column] * vector[column])
                .sum()
        })
        .collect()
}

struct LuFactor {
    original: Vec<f64>,
    lu: Vec<f64>,
    swaps: Vec<usize>,
    dimension: usize,
    log_abs_determinant: f64,
}

impl LuFactor {
    fn new(mut matrix: Vec<f64>, dimension: usize) -> Result<Self, SarError> {
        let original = matrix.clone();
        let scale = matrix
            .iter()
            .map(|value| value.abs())
            .fold(0.0_f64, f64::max)
            .max(1.0);
        let threshold = f64::EPSILON * dimension as f64 * scale;
        let mut swaps = Vec::with_capacity(dimension);
        let mut log_abs_determinant = 0.0;
        for column in 0..dimension {
            let pivot = (column..dimension)
                .max_by(|&left, &right| {
                    matrix[left * dimension + column]
                        .abs()
                        .total_cmp(&matrix[right * dimension + column].abs())
                })
                .expect("nonempty pivot range");
            let pivot_value = matrix[pivot * dimension + column].abs();
            if !pivot_value.is_finite() || pivot_value <= threshold {
                return Err(SarError::InvalidInput(
                    "I-rho*W is singular at the declared rho".into(),
                ));
            }
            swaps.push(pivot);
            if pivot != column {
                for entry in 0..dimension {
                    matrix.swap(column * dimension + entry, pivot * dimension + entry);
                }
            }
            let diagonal = matrix[column * dimension + column];
            log_abs_determinant += diagonal.abs().ln();
            for row in column + 1..dimension {
                matrix[row * dimension + column] /= diagonal;
                let multiplier = matrix[row * dimension + column];
                for entry in column + 1..dimension {
                    matrix[row * dimension + entry] -=
                        multiplier * matrix[column * dimension + entry];
                }
            }
        }
        Ok(Self {
            original,
            lu: matrix,
            swaps,
            dimension,
            log_abs_determinant,
        })
    }

    fn solve(&self, mut right: Vec<f64>) -> Vec<f64> {
        for (column, &pivot) in self.swaps.iter().enumerate() {
            if pivot != column {
                right.swap(column, pivot);
            }
        }
        for row in 0..self.dimension {
            for column in 0..row {
                right[row] -= self.lu[row * self.dimension + column] * right[column];
            }
        }
        for row in (0..self.dimension).rev() {
            for column in row + 1..self.dimension {
                right[row] -= self.lu[row * self.dimension + column] * right[column];
            }
            right[row] /= self.lu[row * self.dimension + row];
        }
        right
    }

    fn inverse(&self) -> Result<Vec<f64>, SarError> {
        let mut inverse = vec![0.0; self.dimension * self.dimension];
        for column in 0..self.dimension {
            let mut unit = vec![0.0; self.dimension];
            unit[column] = 1.0;
            let solution = self.solve(unit);
            for row in 0..self.dimension {
                inverse[row * self.dimension + column] = solution[row];
            }
        }
        if inverse.iter().any(|value| !value.is_finite()) {
            return Err(SarError::Numerical(
                "SAR impact solve produced a non-finite inverse".into(),
            ));
        }
        Ok(inverse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{validate_spatial_weights, SpatialEdge, SpatialWeightsPolicy};

    #[test]
    fn two_region_lag_and_error_match_oracles() {
        let lag = sar_gaussian_log_likelihood(spec(SarModelType::Lag, 0.25)).unwrap();
        let error = sar_gaussian_log_likelihood(spec(SarModelType::Error, 0.25)).unwrap();
        assert!((lag.log_abs_determinant - 0.9375_f64.ln()).abs() <= 1e-12);
        assert!((lag.residual_sum_squares - 0.25).abs() <= 1e-12);
        assert!((lag.log_likelihood + 2.353864256690381).abs() <= 1e-12);
        assert!((lag.impacts[0].direct - 0.8).abs() <= 1e-12);
        assert!((lag.impacts[0].indirect - 0.2).abs() <= 1e-12);
        assert!((lag.impacts[0].total - 1.0).abs() <= 1e-12);
        assert!((error.residual_sum_squares - 0.48828125).abs() <= 1e-12);
        assert!((error.log_likelihood + 2.4366008018292704).abs() <= 1e-12);
        assert!(error.impacts.is_empty());
    }

    #[test]
    fn singular_declared_rho_is_rejected() {
        let error = sar_gaussian_log_likelihood(spec(SarModelType::Lag, 1.0)).unwrap_err();
        assert!(error.to_string().contains("singular at the declared rho"));
    }

    #[test]
    fn partial_pivot_lu_matches_hand_inverse() {
        let factor = LuFactor::new(vec![0.0, 2.0, 1.0, 3.0], 2).unwrap();
        assert!((factor.log_abs_determinant - 2.0_f64.ln()).abs() <= 1e-12);
        let inverse = factor.inverse().unwrap();
        let expected = [-1.5, 1.0, 0.5, 0.0];
        for (actual, expected) in inverse.iter().zip(expected) {
            assert!((actual - expected).abs() <= 1e-12);
        }
    }

    fn spec(model_type: SarModelType, rho: f64) -> SarSpec {
        SarSpec {
            model_type,
            response: vec![1.0, 2.0],
            design: vec![0.0, 1.0],
            predictor_names: vec!["x".into()],
            intercept: 0.5,
            coefficients: vec![0.75],
            rho,
            sigma: 1.2,
            weights: validate_spatial_weights(
                vec!["a".into(), "b".into()],
                vec![edge("a", "b"), edge("b", "a")],
                SpatialWeightsPolicy {
                    symmetry: SymmetryPolicy::NotRequired,
                    diagonal: DiagonalPolicy::Zero,
                    normalization: NormalizationPolicy::RowStandardize,
                },
            )
            .unwrap(),
        }
    }

    fn edge(source: &str, target: &str) -> SpatialEdge {
        SpatialEdge {
            source_region: source.into(),
            target_region: target.into(),
            weight: 1.0,
        }
    }
}

//! Platt scaling and probability-quality summaries.

use marklab_numerics::sigmoid;
use serde::{Deserialize, Serialize};

use crate::logistic::LogisticError;

const MAXIMUM_PLATT_ITERATIONS: usize = 100;

/// `P(positive | margin m) = σ(slope · m + intercept)`.
///
/// Fitted by maximum likelihood to held-out margins with Platt's smoothed targets,
/// `(N₊ + 1) / (N₊ + 2)` for positives and `1 / (N₋ + 2)` for negatives, as in scikit-learn's
/// sigmoid calibration (whose `A` and `B` are `−slope` and `−intercept`).
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct PlattScaling {
    pub slope: f64,
    pub intercept: f64,
}

impl PlattScaling {
    pub fn fit(margins: &[f64], labels: &[bool]) -> Result<Self, LogisticError> {
        let positives = labels.iter().filter(|&&label| label).count();
        let negatives = labels.len() - positives;
        if margins.len() != labels.len() || positives == 0 || negatives == 0 {
            return Err(LogisticError::Invalid(
                "Platt scaling needs margins of both classes".to_owned(),
            ));
        }
        if !margins.iter().all(|margin| margin.is_finite()) {
            return Err(LogisticError::Invalid(
                "Platt scaling needs finite margins".to_owned(),
            ));
        }
        if margins.iter().all(|&margin| margin == margins[0]) {
            return Err(LogisticError::Numerical(
                "every held-out margin is equal, so Platt scaling is undetermined".to_owned(),
            ));
        }
        let high = (positives as f64 + 1.0) / (positives as f64 + 2.0);
        let low = 1.0 / (negatives as f64 + 2.0);
        let targets: Vec<f64> = labels
            .iter()
            .map(|&label| if label { high } else { low })
            .collect();
        let loss = |slope: f64, intercept: f64| -> f64 {
            margins
                .iter()
                .zip(&targets)
                .map(|(&margin, &target)| {
                    let z = slope * margin + intercept;
                    // −[t log σ(z) + (1 − t) log σ(−z)] = softplus(z) − t z.
                    z.max(0.0) + (-z.abs()).exp().ln_1p() - target * z
                })
                .sum()
        };
        let (mut slope, mut intercept) = (
            0.0,
            ((positives as f64 + 1.0) / (negatives as f64 + 1.0)).ln(),
        );
        let mut value = loss(slope, intercept);
        let mut previous_decrement = f64::INFINITY;
        for _ in 0..MAXIMUM_PLATT_ITERATIONS {
            let (mut g0, mut g1, mut h00, mut h01, mut h11) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for (&margin, &target) in margins.iter().zip(&targets) {
                let probability = sigmoid(slope * margin + intercept);
                let residual = probability - target;
                let curvature = probability * (1.0 - probability);
                g0 += residual * margin;
                g1 += residual;
                h00 += curvature * margin * margin;
                h01 += curvature * margin;
                h11 += curvature;
            }
            let determinant = h00 * h11 - h01 * h01;
            if !(determinant.is_finite() && determinant > 0.0) {
                return Err(LogisticError::Numerical(
                    "the Platt scaling Hessian is singular".to_owned(),
                ));
            }
            let step0 = -(h11 * g0 - h01 * g1) / determinant;
            let step1 = -(h00 * g1 - h01 * g0) / determinant;
            let decrement = -(g0 * step0 + g1 * step1);
            let reference = value.max(1.0);
            if decrement <= 1e-12 * reference {
                // Close to the minimum, full steps converge quadratically. Stop when the
                // predicted decrease is negligible or rounding stops it from shrinking.
                let next = loss(slope + step0, intercept + step1);
                if next > value + 1e-12 * reference {
                    return Ok(Self { slope, intercept });
                }
                (slope, intercept, value) = (slope + step0, intercept + step1, next);
                if decrement <= 1e-28 * reference || decrement >= previous_decrement {
                    return Ok(Self { slope, intercept });
                }
                previous_decrement = decrement;
                continue;
            }
            previous_decrement = f64::INFINITY;
            let mut length = 1.0;
            loop {
                let (next_slope, next_intercept) =
                    (slope + length * step0, intercept + length * step1);
                let next = loss(next_slope, next_intercept);
                if next <= value - 1e-4 * length * decrement {
                    (slope, intercept, value) = (next_slope, next_intercept, next);
                    break;
                }
                length *= 0.5;
                if length < 1e-10 {
                    return Err(LogisticError::Numerical(
                        "the Platt scaling line search found no decrease".to_owned(),
                    ));
                }
            }
        }
        Err(LogisticError::Numerical(format!(
            "Platt scaling did not converge in {MAXIMUM_PLATT_ITERATIONS} iterations"
        )))
    }

    pub fn probability(&self, margin: f64) -> f64 {
        sigmoid(self.slope * margin + self.intercept)
    }

    pub(crate) fn is_finite(&self) -> bool {
        self.slope.is_finite() && self.intercept.is_finite()
    }
}

/// One of ten equal-width probability bins.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ReliabilityBin {
    pub lower: f64,
    pub upper: f64,
    pub rows: usize,
    /// Mean predicted probability of the rows in the bin; `None` when it is empty.
    pub mean_probability: Option<f64>,
    /// Fraction of positive rows in the bin; `None` when it is empty.
    pub observed_fraction: Option<f64>,
}

/// Mean squared difference between probabilities and outcomes.
pub fn brier_score(labels: &[bool], probabilities: &[f64]) -> f64 {
    labels
        .iter()
        .zip(probabilities)
        .map(|(&label, &probability)| (probability - f64::from(u8::from(label))).powi(2))
        .sum::<f64>()
        / labels.len() as f64
}

/// Predicted against observed frequency in ten bins `[0, 0.1), …, [0.9, 1]`.
pub fn reliability(labels: &[bool], probabilities: &[f64]) -> Vec<ReliabilityBin> {
    let mut rows = [0usize; 10];
    let mut predicted = [0.0; 10];
    let mut positives = [0usize; 10];
    for (&label, &probability) in labels.iter().zip(probabilities) {
        let bin = ((probability * 10.0) as usize).min(9);
        rows[bin] += 1;
        predicted[bin] += probability;
        positives[bin] += usize::from(label);
    }
    (0..10)
        .map(|bin| ReliabilityBin {
            lower: bin as f64 / 10.0,
            upper: (bin + 1) as f64 / 10.0,
            rows: rows[bin],
            mean_probability: (rows[bin] > 0).then(|| predicted[bin] / rows[bin] as f64),
            observed_fraction: (rows[bin] > 0).then(|| positives[bin] as f64 / rows[bin] as f64),
        })
        .collect()
}

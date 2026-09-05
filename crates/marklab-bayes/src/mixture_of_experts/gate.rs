use marklab_numerics::{minimize_lbfgs, NumericsError};
use std::time::Instant;

pub(super) struct PreparedGate {
    pub features: Vec<f64>,
    pub probabilities: Vec<f64>,
    pub labels: Vec<f64>,
    pub feature_columns: usize,
    pub experts: usize,
}

pub(super) fn fit(
    prepared: &PreparedGate,
    l2_penalty: f64,
    entropy_regularization: f64,
    deadline: Instant,
) -> Result<Vec<f64>, NumericsError> {
    let columns = prepared.feature_columns + 1;
    let initial = vec![0.0; prepared.experts * columns];
    minimize_lbfgs(
        &initial,
        2_000,
        1e-7,
        1e-12,
        10,
        50,
        |parameters, gradient| {
            objective(
                prepared,
                parameters,
                gradient,
                l2_penalty,
                entropy_regularization,
                deadline,
            )
        },
    )
}

pub(super) fn weights(
    coefficients: &[f64],
    columns: usize,
    features: &[f64],
    probabilities: &[Option<f64>],
    output: &mut [f64],
) -> Result<(), NumericsError> {
    let mut maximum = f64::NEG_INFINITY;
    for (expert, probability) in probabilities.iter().enumerate() {
        let value = if probability.is_some() {
            coefficients[expert * columns]
                + coefficients[expert * columns + 1..(expert + 1) * columns]
                    .iter()
                    .zip(features)
                    .map(|(coefficient, feature)| coefficient * feature)
                    .sum::<f64>()
        } else {
            f64::NEG_INFINITY
        };
        if probability.is_some() && !value.is_finite() {
            return Err(NumericsError::Numerical(
                "mixture gate produced a nonfinite available logit".into(),
            ));
        }
        output[expert] = value;
        maximum = maximum.max(value);
    }
    let mut total = 0.0;
    for value in output.iter_mut() {
        *value = if value.is_finite() {
            (*value - maximum).exp()
        } else {
            0.0
        };
        total += *value;
    }
    if !total.is_finite() || total <= 0.0 {
        return Err(NumericsError::Numerical(
            "mixture gate produced invalid available weights".into(),
        ));
    }
    for value in output {
        *value /= total;
    }
    Ok(())
}

fn objective(
    prepared: &PreparedGate,
    parameters: &[f64],
    gradient: &mut [f64],
    l2_penalty: f64,
    entropy_regularization: f64,
    deadline: Instant,
) -> Result<f64, NumericsError> {
    let columns = prepared.feature_columns + 1;
    gradient.fill(0.0);
    let mut objective = 0.0;
    let mut logits = vec![0.0; prepared.experts];
    for row_index in 0..prepared.labels.len() {
        if row_index % 256 == 0 && Instant::now() >= deadline {
            return Err(NumericsError::Resource(
                "mixture gate deadline exceeded".into(),
            ));
        }
        let features = &prepared.features
            [row_index * prepared.feature_columns..(row_index + 1) * prepared.feature_columns];
        let probabilities = &prepared.probabilities
            [row_index * prepared.experts..(row_index + 1) * prepared.experts];
        let mut maximum = f64::NEG_INFINITY;
        for expert in 0..prepared.experts {
            let probability = probabilities[expert];
            let value = if probability.is_nan() {
                f64::NEG_INFINITY
            } else {
                parameters[expert * columns]
                    + parameters[expert * columns + 1..(expert + 1) * columns]
                        .iter()
                        .zip(features)
                        .map(|(coefficient, feature)| coefficient * feature)
                        .sum::<f64>()
            };
            if !probability.is_nan() && !value.is_finite() {
                return Err(NumericsError::Numerical(
                    "mixture gate objective produced a nonfinite available logit".into(),
                ));
            }
            logits[expert] = value;
            maximum = maximum.max(value);
        }
        let mut total = 0.0;
        for value in &mut logits {
            *value = if value.is_finite() {
                (*value - maximum).exp()
            } else {
                0.0
            };
            total += *value;
        }
        if !total.is_finite() || total <= 0.0 {
            return Err(NumericsError::Numerical(
                "mixture gate objective produced invalid weights".into(),
            ));
        }
        for value in &mut logits {
            *value /= total;
        }
        let mixture = logits
            .iter()
            .zip(probabilities)
            .filter(|(_, probability)| !probability.is_nan())
            .map(|(weight, probability)| weight * probability)
            .sum::<f64>();
        let clipped = mixture.clamp(1e-12, 1.0 - 1e-12);
        let label = prepared.labels[row_index];
        objective -= label * clipped.ln() + (1.0 - label) * (1.0 - clipped).ln();
        let weight_log_weight = logits
            .iter()
            .filter(|weight| **weight > 0.0)
            .map(|weight| weight * weight.ln())
            .sum::<f64>();
        objective += entropy_regularization * weight_log_weight;
        let loss_derivative = if mixture > 1e-12 && mixture < 1.0 - 1e-12 {
            (mixture - label) / (mixture * (1.0 - mixture))
        } else {
            0.0
        };
        for expert in 0..prepared.experts {
            let weight = logits[expert];
            if weight == 0.0 {
                continue;
            }
            let probability = probabilities[expert];
            let derivative = loss_derivative * weight * (probability - mixture)
                + entropy_regularization * weight * (weight.ln() - weight_log_weight);
            let offset = expert * columns;
            gradient[offset] += derivative;
            for (value, feature) in gradient[offset + 1..offset + columns]
                .iter_mut()
                .zip(features)
            {
                *value += derivative * feature;
            }
        }
    }
    for (gradient, parameter) in gradient.iter_mut().zip(parameters) {
        objective += 0.5 * l2_penalty * parameter * parameter;
        *gradient += l2_penalty * parameter;
    }
    Ok(objective)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytic_gradient_matches_central_difference() {
        let prepared = PreparedGate {
            features: vec![0.2, 1.0, 1.0, -0.7, 1.0, 0.0],
            probabilities: vec![0.8, 0.3, 0.2, f64::NAN],
            labels: vec![1.0, 0.0],
            feature_columns: 3,
            experts: 2,
        };
        let parameters = vec![0.1, -0.2, 0.3, -0.1, -0.4, 0.2, 0.1, 0.5];
        let deadline = Instant::now() + std::time::Duration::from_secs(1);
        let mut gradient = vec![0.0; parameters.len()];
        objective(&prepared, &parameters, &mut gradient, 0.1, 0.02, deadline).unwrap();
        for index in 0..parameters.len() {
            let mut lower = parameters.clone();
            let mut upper = parameters.clone();
            lower[index] -= 1e-5;
            upper[index] += 1e-5;
            let low = objective(
                &prepared,
                &lower,
                &mut vec![0.0; parameters.len()],
                0.1,
                0.02,
                deadline,
            )
            .unwrap();
            let high = objective(
                &prepared,
                &upper,
                &mut vec![0.0; parameters.len()],
                0.1,
                0.02,
                deadline,
            )
            .unwrap();
            let finite = (high - low) / 2e-5;
            assert!((finite - gradient[index]).abs() < 1e-9, "{index}");
        }
    }

    #[test]
    fn available_nonfinite_logit_is_not_reinterpreted_as_missing() {
        let mut output = [0.0; 2];
        let result = weights(
            &[0.0, f64::MAX, -f64::MAX, 0.0, 0.0, 0.0],
            3,
            &[2.0, 2.0],
            &[Some(0.2), Some(0.8)],
            &mut output,
        );
        assert!(matches!(result, Err(NumericsError::Numerical(_))));
        weights(
            &[0.0, f64::MAX, -f64::MAX, 0.0, 0.0, 0.0],
            3,
            &[2.0, 2.0],
            &[None, Some(0.8)],
            &mut output,
        )
        .unwrap();
        assert_eq!(output, [0.0, 1.0]);
    }
}

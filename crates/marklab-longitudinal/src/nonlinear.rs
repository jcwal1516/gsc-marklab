use serde::{Deserialize, Serialize};

use crate::LongitudinalError;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NonlinearFilterMethod {
    Ekf,
    Ukf,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuadraticFunction {
    pub intercept: f64,
    pub linear: f64,
    pub quadratic: f64,
}

impl QuadraticFunction {
    fn evaluate(self, value: f64) -> f64 {
        self.intercept + value * (self.linear + value * self.quadratic)
    }

    fn derivative(self, value: f64) -> f64 {
        self.linear + 2.0 * self.quadratic * value
    }

    fn is_finite(self) -> bool {
        self.intercept.is_finite() && self.linear.is_finite() && self.quadratic.is_finite()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonlinearFilterStep {
    pub transition: QuadraticFunction,
    pub process_variance: f64,
    pub observation: QuadraticFunction,
    pub observation_variance: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarNonlinearFilterSpec {
    pub method: NonlinearFilterMethod,
    pub observations: Vec<Option<f64>>,
    pub steps: Vec<NonlinearFilterStep>,
    pub initial_mean: f64,
    pub initial_variance: f64,
    pub ukf_alpha: f64,
    pub ukf_beta: f64,
    pub ukf_kappa: f64,
    pub maximum_time_steps: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScalarMoment {
    pub time_index: usize,
    pub mean: f64,
    pub variance: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NonlinearStepDiagnostics {
    pub time_index: usize,
    pub observed: bool,
    pub innovation: Option<f64>,
    pub innovation_variance: Option<f64>,
    pub gain: Option<f64>,
    pub transition_derivative: Option<f64>,
    pub observation_derivative: Option<f64>,
    pub transition_sigma_spread: Option<f64>,
    pub observation_sigma_spread: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScalarNonlinearFilterResult {
    pub format: &'static str,
    pub version: u32,
    pub method: NonlinearFilterMethod,
    pub time_steps: usize,
    pub observed_updates: usize,
    pub log_likelihood: f64,
    pub predicted_states: Vec<ScalarMoment>,
    pub filtered_states: Vec<ScalarMoment>,
    pub diagnostics: Vec<NonlinearStepDiagnostics>,
    pub claim_status: &'static str,
}

struct SigmaPoints {
    values: [f64; 3],
    mean_weights: [f64; 3],
    covariance_weights: [f64; 3],
    spread: f64,
}

pub fn nonlinear_gaussian_filter(
    spec: ScalarNonlinearFilterSpec,
) -> Result<ScalarNonlinearFilterResult, LongitudinalError> {
    validate(&spec)?;
    let mut mean = spec.initial_mean;
    let mut variance = spec.initial_variance;
    let mut predicted_states = Vec::with_capacity(spec.steps.len());
    let mut filtered_states = Vec::with_capacity(spec.steps.len());
    let mut diagnostics = Vec::with_capacity(spec.steps.len());
    let mut log_likelihood = 0.0;
    let mut observed_updates = 0;

    for (time_index, (step, observation)) in spec.steps.iter().zip(&spec.observations).enumerate() {
        let (predicted_mean, predicted_variance, transition_derivative, transition_spread) =
            match spec.method {
                NonlinearFilterMethod::Ekf => {
                    let derivative = step.transition.derivative(mean);
                    (
                        step.transition.evaluate(mean),
                        derivative * derivative * variance + step.process_variance,
                        Some(derivative),
                        None,
                    )
                }
                NonlinearFilterMethod::Ukf => {
                    let sigma = sigma_points(
                        mean,
                        variance,
                        spec.ukf_alpha,
                        spec.ukf_beta,
                        spec.ukf_kappa,
                    )?;
                    let transformed = sigma.values.map(|point| step.transition.evaluate(point));
                    let transformed_mean = weighted_mean(transformed, sigma.mean_weights);
                    let transformed_variance =
                        weighted_variance(transformed, transformed_mean, sigma.covariance_weights)
                            + step.process_variance;
                    (
                        transformed_mean,
                        transformed_variance,
                        None,
                        Some(sigma.spread),
                    )
                }
            };
        let predicted_variance = checked_variance(predicted_variance, "predicted variance")?;
        ensure_finite(predicted_mean, predicted_variance, "predicted state")?;
        predicted_states.push(ScalarMoment {
            time_index,
            mean: predicted_mean,
            variance: predicted_variance,
        });

        let mut step_diagnostics = NonlinearStepDiagnostics {
            time_index,
            observed: observation.is_some(),
            innovation: None,
            innovation_variance: None,
            gain: None,
            transition_derivative,
            observation_derivative: None,
            transition_sigma_spread: transition_spread,
            observation_sigma_spread: None,
        };
        if let Some(observed) = observation {
            let (predicted_observation, innovation_variance, cross_covariance) = match spec.method {
                NonlinearFilterMethod::Ekf => {
                    let derivative = step.observation.derivative(predicted_mean);
                    step_diagnostics.observation_derivative = Some(derivative);
                    (
                        step.observation.evaluate(predicted_mean),
                        derivative * derivative * predicted_variance + step.observation_variance,
                        predicted_variance * derivative,
                    )
                }
                NonlinearFilterMethod::Ukf => {
                    let sigma = sigma_points(
                        predicted_mean,
                        predicted_variance,
                        spec.ukf_alpha,
                        spec.ukf_beta,
                        spec.ukf_kappa,
                    )?;
                    step_diagnostics.observation_sigma_spread = Some(sigma.spread);
                    let transformed = sigma.values.map(|point| step.observation.evaluate(point));
                    let observation_mean = weighted_mean(transformed, sigma.mean_weights);
                    let observation_variance =
                        weighted_variance(transformed, observation_mean, sigma.covariance_weights)
                            + step.observation_variance;
                    let cross = (0..3)
                        .map(|index| {
                            sigma.covariance_weights[index]
                                * (sigma.values[index] - predicted_mean)
                                * (transformed[index] - observation_mean)
                        })
                        .sum();
                    (observation_mean, observation_variance, cross)
                }
            };
            if !innovation_variance.is_finite() || innovation_variance <= 0.0 {
                return Err(LongitudinalError::Numerical(format!(
                    "innovation variance at step {time_index} must be positive and finite"
                )));
            }
            let innovation = observed - predicted_observation;
            let gain = cross_covariance / innovation_variance;
            mean = predicted_mean + gain * innovation;
            let updated_variance = match spec.method {
                NonlinearFilterMethod::Ekf => {
                    let derivative = step_diagnostics
                        .observation_derivative
                        .expect("EKF observation derivative is retained");
                    (1.0 - gain * derivative).powi(2) * predicted_variance
                        + gain * gain * step.observation_variance
                }
                NonlinearFilterMethod::Ukf => {
                    predicted_variance - gain * innovation_variance * gain
                }
            };
            variance = checked_variance(updated_variance, "filtered variance")?;
            ensure_finite(mean, variance, "filtered state")?;
            log_likelihood += -0.5
                * ((2.0 * std::f64::consts::PI * innovation_variance).ln()
                    + innovation * innovation / innovation_variance);
            observed_updates += 1;
            step_diagnostics.innovation = Some(innovation);
            step_diagnostics.innovation_variance = Some(innovation_variance);
            step_diagnostics.gain = Some(gain);
        } else {
            mean = predicted_mean;
            variance = predicted_variance;
        }
        filtered_states.push(ScalarMoment {
            time_index,
            mean,
            variance,
        });
        diagnostics.push(step_diagnostics);
    }
    if !log_likelihood.is_finite() {
        return Err(LongitudinalError::Numerical(
            "innovation log likelihood is not finite".into(),
        ));
    }
    Ok(ScalarNonlinearFilterResult {
        format: "marklab.scalar_nonlinear_gaussian_filter",
        version: 1,
        method: spec.method,
        time_steps: spec.steps.len(),
        observed_updates,
        log_likelihood,
        predicted_states,
        filtered_states,
        diagnostics,
        claim_status: "scalar_gaussian_moment_approximation_only",
    })
}

fn sigma_points(
    mean: f64,
    variance: f64,
    alpha: f64,
    beta: f64,
    kappa: f64,
) -> Result<SigmaPoints, LongitudinalError> {
    let denominator = alpha * alpha * (1.0 + kappa);
    if !denominator.is_finite() || denominator <= 0.0 {
        return Err(LongitudinalError::Invalid(
            "UKF alpha^2*(1+kappa) must be positive and finite".into(),
        ));
    }
    let lambda = denominator - 1.0;
    let spread = (denominator * variance).sqrt();
    let side_weight = 0.5 / denominator;
    Ok(SigmaPoints {
        values: [mean, mean + spread, mean - spread],
        mean_weights: [lambda / denominator, side_weight, side_weight],
        covariance_weights: [
            lambda / denominator + (1.0 - alpha * alpha + beta),
            side_weight,
            side_weight,
        ],
        spread,
    })
}

fn weighted_mean(values: [f64; 3], weights: [f64; 3]) -> f64 {
    (0..3).map(|index| weights[index] * values[index]).sum()
}

fn weighted_variance(values: [f64; 3], mean: f64, weights: [f64; 3]) -> f64 {
    (0..3)
        .map(|index| weights[index] * (values[index] - mean).powi(2))
        .sum()
}

fn checked_variance(value: f64, name: &str) -> Result<f64, LongitudinalError> {
    if !value.is_finite() || value < -1.0e-12 {
        return Err(LongitudinalError::Numerical(format!(
            "{name} became negative or non-finite"
        )));
    }
    Ok(value.max(0.0))
}

fn ensure_finite(mean: f64, variance: f64, name: &str) -> Result<(), LongitudinalError> {
    if !mean.is_finite() || !variance.is_finite() {
        return Err(LongitudinalError::Numerical(format!(
            "{name} is not finite"
        )));
    }
    Ok(())
}

fn validate(spec: &ScalarNonlinearFilterSpec) -> Result<(), LongitudinalError> {
    if spec.observations.is_empty()
        || spec.observations.len() != spec.steps.len()
        || spec.steps.len() > spec.maximum_time_steps
    {
        return Err(LongitudinalError::Resource(
            "observations/steps must have the same positive length within maximum_time_steps"
                .into(),
        ));
    }
    if !spec.initial_mean.is_finite()
        || !spec.initial_variance.is_finite()
        || spec.initial_variance <= 0.0
    {
        return Err(LongitudinalError::Invalid(
            "initial mean must be finite and initial variance positive finite".into(),
        ));
    }
    if spec
        .observations
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err(LongitudinalError::Invalid(
            "observations must be finite or null".into(),
        ));
    }
    for (index, step) in spec.steps.iter().enumerate() {
        if !step.transition.is_finite()
            || !step.observation.is_finite()
            || !step.process_variance.is_finite()
            || step.process_variance < 0.0
            || !step.observation_variance.is_finite()
            || step.observation_variance <= 0.0
        {
            return Err(LongitudinalError::Invalid(format!(
                "step {index} functions/variances violate finite nonnegative-process/positive-observation requirements"
            )));
        }
    }
    if matches!(spec.method, NonlinearFilterMethod::Ukf)
        && (!spec.ukf_alpha.is_finite()
            || spec.ukf_alpha <= 0.0
            || !spec.ukf_beta.is_finite()
            || !spec.ukf_kappa.is_finite()
            || spec.ukf_alpha * spec.ukf_alpha * (1.0 + spec.ukf_kappa) <= 0.0)
    {
        return Err(LongitudinalError::Invalid(
            "UKF settings must be finite with positive alpha and alpha^2*(1+kappa)".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_nonlinear_observation_performs_prediction_only() {
        let result = nonlinear_gaussian_filter(ScalarNonlinearFilterSpec {
            method: NonlinearFilterMethod::Ekf,
            observations: vec![None],
            steps: vec![NonlinearFilterStep {
                transition: QuadraticFunction {
                    intercept: 0.0,
                    linear: 1.0,
                    quadratic: 0.1,
                },
                process_variance: 0.2,
                observation: QuadraticFunction {
                    intercept: 0.0,
                    linear: 1.0,
                    quadratic: 0.0,
                },
                observation_variance: 1.0,
            }],
            initial_mean: 2.0,
            initial_variance: 1.0,
            ukf_alpha: 0.5,
            ukf_beta: 2.0,
            ukf_kappa: 0.0,
            maximum_time_steps: 1,
        })
        .unwrap();
        assert_eq!(result.observed_updates, 0);
        assert_eq!(result.predicted_states[0].mean, 2.4);
        assert_eq!(
            result.predicted_states[0].mean,
            result.filtered_states[0].mean
        );
        assert_eq!(result.log_likelihood, 0.0);
    }
}

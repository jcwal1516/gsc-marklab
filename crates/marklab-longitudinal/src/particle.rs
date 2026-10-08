use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

use crate::{LongitudinalError, QuadraticFunction};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticleFilterStepSpec {
    pub transition: QuadraticFunction,
    pub process_sd: f64,
    pub observation: QuadraticFunction,
    pub observation_sd: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarParticleSmootherSpec {
    pub observations: Vec<Option<f64>>,
    pub steps: Vec<ParticleFilterStepSpec>,
    pub initial_mean: f64,
    pub initial_sd: f64,
    pub particles: usize,
    pub ess_resampling_fraction: f64,
    pub smoothed_trajectories: usize,
    pub seed: u64,
    pub maximum_particle_steps: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParticleFilteringStep {
    pub time_index: usize,
    pub particles: Vec<f64>,
    pub weights: Vec<f64>,
    pub ancestry: Vec<usize>,
    pub effective_sample_size: f64,
    pub weighted_mean: f64,
    pub weighted_variance: f64,
    pub likelihood_increment: f64,
    pub resampled: bool,
    pub observed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SmoothedParticleTrajectory {
    pub trajectory_index: usize,
    pub terminal_particle_index: usize,
    pub states: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScalarParticleSmootherResult {
    pub format: &'static str,
    pub version: u32,
    pub algorithm: &'static str,
    pub particle_steps: u64,
    pub marginal_log_likelihood: f64,
    pub filtering_steps: Vec<ParticleFilteringStep>,
    pub smoothed_trajectories: Vec<SmoothedParticleTrajectory>,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

pub fn particle_filter_and_smooth(
    spec: ScalarParticleSmootherSpec,
) -> Result<ScalarParticleSmootherResult, LongitudinalError> {
    let particle_steps = validate(&spec)?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut particles =
        sample_normal_vector(spec.initial_mean, spec.initial_sd, spec.particles, &mut rng)?;
    let mut weights = vec![1.0 / spec.particles as f64; spec.particles];
    let mut filtering_steps = Vec::with_capacity(spec.steps.len());
    let mut marginal_log_likelihood = 0.0;

    for (time_index, (step, observation)) in spec.steps.iter().zip(&spec.observations).enumerate() {
        let mut proposed = Vec::with_capacity(spec.particles);
        for &particle in &particles {
            let transition_mean = evaluate(step.transition, particle);
            let noise = sample_normal(0.0, step.process_sd, &mut rng)?;
            let value = transition_mean + noise;
            if !value.is_finite() {
                return Err(LongitudinalError::Numerical(format!(
                    "particle transition at step {time_index} is not finite"
                )));
            }
            proposed.push(value);
        }
        let (normalized, likelihood_increment) = if let Some(observed) = observation {
            normalize_log_weights(
                &proposed,
                &weights,
                *observed,
                step.observation,
                step.observation_sd,
            )?
        } else {
            (weights.clone(), 0.0)
        };
        marginal_log_likelihood += likelihood_increment;
        let ess = 1.0 / normalized.iter().map(|weight| weight * weight).sum::<f64>();
        let weighted_mean = proposed
            .iter()
            .zip(&normalized)
            .map(|(value, weight)| value * weight)
            .sum::<f64>();
        let weighted_variance = proposed
            .iter()
            .zip(&normalized)
            .map(|(value, weight)| weight * (value - weighted_mean).powi(2))
            .sum::<f64>();
        let resampled = ess < spec.ess_resampling_fraction * spec.particles as f64;
        let ancestry = if resampled {
            systematic_resample(&normalized, &mut rng)
        } else {
            (0..spec.particles).collect()
        };
        if resampled {
            particles = ancestry.iter().map(|&index| proposed[index]).collect();
            weights.fill(1.0 / spec.particles as f64);
        } else {
            particles = proposed;
            weights = normalized;
        }
        filtering_steps.push(ParticleFilteringStep {
            time_index,
            particles: particles.clone(),
            weights: weights.clone(),
            ancestry,
            effective_sample_size: ess,
            weighted_mean,
            weighted_variance,
            likelihood_increment,
            resampled,
            observed: observation.is_some(),
        });
    }

    let terminal_weights = &filtering_steps
        .last()
        .expect("validated nonempty steps")
        .weights;
    let mut smoothed_trajectories = Vec::with_capacity(spec.smoothed_trajectories);
    for trajectory_index in 0..spec.smoothed_trajectories {
        let terminal_particle_index = categorical_index(terminal_weights, rng.gen::<f64>());
        let mut particle_index = terminal_particle_index;
        let mut states = vec![0.0; filtering_steps.len()];
        for time_index in (0..filtering_steps.len()).rev() {
            states[time_index] = filtering_steps[time_index].particles[particle_index];
            if time_index > 0 {
                particle_index = filtering_steps[time_index].ancestry[particle_index];
            }
        }
        smoothed_trajectories.push(SmoothedParticleTrajectory {
            trajectory_index,
            terminal_particle_index,
            states,
        });
    }
    Ok(ScalarParticleSmootherResult {
        format: "marklab.scalar_bootstrap_particle_smoother",
        version: 1,
        algorithm: "bootstrap_sequential_weights_systematic_resampling_ancestry_trace",
        particle_steps,
        marginal_log_likelihood,
        filtering_steps,
        smoothed_trajectories,
        random_seed_namespace: "scalar_particle_smoother_v1_chacha20",
        claim_status: "finite_particle_approximation_only",
    })
}

fn validate(spec: &ScalarParticleSmootherSpec) -> Result<u64, LongitudinalError> {
    if spec.steps.is_empty() || spec.steps.len() != spec.observations.len() {
        return Err(LongitudinalError::Invalid(
            "steps and observations must have the same positive length".into(),
        ));
    }
    if spec.particles == 0 || spec.smoothed_trajectories == 0 {
        return Err(LongitudinalError::Resource(
            "particle and smoothed-trajectory counts must be positive".into(),
        ));
    }
    let work = u64::try_from(spec.steps.len())
        .ok()
        .and_then(|steps| {
            u64::try_from(spec.particles)
                .ok()
                .and_then(|n| steps.checked_mul(n))
        })
        .ok_or_else(|| LongitudinalError::Resource("particle-step work overflowed".into()))?;
    if work > spec.maximum_particle_steps {
        return Err(LongitudinalError::Resource(format!(
            "particle steps {work} exceed declared maximum {}",
            spec.maximum_particle_steps
        )));
    }
    if !spec.initial_mean.is_finite()
        || !spec.initial_sd.is_finite()
        || spec.initial_sd < 0.0
        || !spec.ess_resampling_fraction.is_finite()
        || !(0.0..=1.0).contains(&spec.ess_resampling_fraction)
        || spec
            .observations
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
    {
        return Err(LongitudinalError::Invalid(
            "initial law, observations, or ESS fraction is invalid".into(),
        ));
    }
    for step in &spec.steps {
        if !step.process_sd.is_finite()
            || step.process_sd < 0.0
            || !step.observation_sd.is_finite()
            || step.observation_sd <= 0.0
            || [
                step.transition.intercept,
                step.transition.linear,
                step.transition.quadratic,
                step.observation.intercept,
                step.observation.linear,
                step.observation.quadratic,
            ]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(LongitudinalError::Invalid(
                "particle step functions/noise scales are invalid".into(),
            ));
        }
    }
    Ok(work)
}

fn evaluate(function: QuadraticFunction, value: f64) -> f64 {
    function.intercept + value * (function.linear + value * function.quadratic)
}

fn sample_normal_vector(
    mean: f64,
    sd: f64,
    count: usize,
    rng: &mut ChaCha20Rng,
) -> Result<Vec<f64>, LongitudinalError> {
    (0..count).map(|_| sample_normal(mean, sd, rng)).collect()
}

fn sample_normal(mean: f64, sd: f64, rng: &mut ChaCha20Rng) -> Result<f64, LongitudinalError> {
    if sd == 0.0 {
        return Ok(mean);
    }
    Normal::new(mean, sd)
        .map_err(|error| LongitudinalError::Invalid(error.to_string()))
        .map(|distribution| distribution.sample(rng))
}

fn normalize_log_weights(
    particles: &[f64],
    prior_weights: &[f64],
    observed: f64,
    observation: QuadraticFunction,
    observation_sd: f64,
) -> Result<(Vec<f64>, f64), LongitudinalError> {
    let normalizer = (observation_sd * (2.0 * std::f64::consts::PI).sqrt()).ln();
    let logs = particles
        .iter()
        .zip(prior_weights)
        .map(|(&particle, &weight)| {
            let residual = (observed - evaluate(observation, particle)) / observation_sd;
            weight.ln() - normalizer - 0.5 * residual * residual
        })
        .collect::<Vec<_>>();
    let maximum = logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let sum = logs
        .iter()
        .map(|value| (value - maximum).exp())
        .sum::<f64>();
    if !maximum.is_finite() || !sum.is_finite() || sum <= 0.0 {
        return Err(LongitudinalError::Numerical(
            "particle weights cannot be normalized".into(),
        ));
    }
    let log_evidence = maximum + sum.ln();
    Ok((
        logs.iter()
            .map(|value| (value - log_evidence).exp())
            .collect(),
        log_evidence,
    ))
}

fn systematic_resample(weights: &[f64], rng: &mut ChaCha20Rng) -> Vec<usize> {
    let count = weights.len();
    let offset = rng.gen::<f64>() / count as f64;
    let mut result = Vec::with_capacity(count);
    let mut index = 0;
    let mut cumulative = weights[0];
    for position in 0..count {
        let target = offset + position as f64 / count as f64;
        while target > cumulative && index + 1 < count {
            index += 1;
            cumulative += weights[index];
        }
        result.push(index);
    }
    result
}

fn categorical_index(weights: &[f64], target: f64) -> usize {
    let mut cumulative = 0.0;
    for (index, weight) in weights.iter().enumerate() {
        cumulative += weight;
        if target < cumulative {
            return index;
        }
    }
    weights.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concentrated_observation_triggers_systematic_resampling() {
        let result = particle_filter_and_smooth(ScalarParticleSmootherSpec {
            observations: vec![Some(10.0)],
            steps: vec![ParticleFilterStepSpec {
                transition: QuadraticFunction {
                    intercept: 0.0,
                    linear: 1.0,
                    quadratic: 0.0,
                },
                process_sd: 0.0,
                observation: QuadraticFunction {
                    intercept: 0.0,
                    linear: 1.0,
                    quadratic: 0.0,
                },
                observation_sd: 0.1,
            }],
            initial_mean: 0.0,
            initial_sd: 1.0,
            particles: 128,
            ess_resampling_fraction: 1.0,
            smoothed_trajectories: 2,
            seed: 17,
            maximum_particle_steps: 128,
        })
        .unwrap();
        let step = &result.filtering_steps[0];
        assert!(step.resampled);
        assert!(step.effective_sample_size < 128.0);
        assert!(step.ancestry.iter().all(|&index| index < 128));
        assert!(step
            .weights
            .iter()
            .all(|weight| (*weight - 1.0 / 128.0).abs() < f64::EPSILON));
        assert!(result.marginal_log_likelihood.is_finite());
    }
}

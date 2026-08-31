use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Deserializer, Serialize};

use crate::CausalError;

const MAXIMUM_LIKELIHOOD_EVALUATIONS: u64 = 250_000_000;
const MAXIMUM_OUTER_VALUES: usize = 1_000_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GaussianEigSpec {
    pub candidate_id: String,
    pub prior_mean: f64,
    pub prior_sd: f64,
    pub sensitivity: f64,
    pub noise_sd: f64,
    pub outer_samples: usize,
    pub inner_samples: usize,
    pub seed: u64,
    pub maximum_likelihood_evaluations: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GaussianEigOuterValue {
    pub outer_index: usize,
    pub theta: f64,
    pub simulated_observation: f64,
    pub log_numerator: f64,
    pub log_denominator: f64,
    pub information_value: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GaussianEigResult {
    pub format: &'static str,
    pub version: u32,
    pub candidate_id: String,
    pub prior: &'static str,
    pub simulator: &'static str,
    pub likelihood: &'static str,
    pub outer_samples: usize,
    pub inner_samples: usize,
    pub outer_values: Vec<GaussianEigOuterValue>,
    pub estimate: f64,
    pub monte_carlo_se: f64,
    pub analytic_eig: f64,
    pub signed_bias_against_analytic: f64,
    pub absolute_bias_against_analytic: f64,
    pub likelihood_evaluations: u64,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

impl<'de> Deserialize<'de> for GaussianEigResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Owned {
            format: String,
            version: u32,
            candidate_id: String,
            prior: String,
            simulator: String,
            likelihood: String,
            outer_samples: usize,
            inner_samples: usize,
            outer_values: Vec<GaussianEigOuterValue>,
            estimate: f64,
            monte_carlo_se: f64,
            analytic_eig: f64,
            signed_bias_against_analytic: f64,
            absolute_bias_against_analytic: f64,
            likelihood_evaluations: u64,
            random_seed_namespace: String,
            claim_status: String,
        }
        let owned = Owned::deserialize(deserializer)?;
        if owned.format != "marklab.gaussian_expected_information_gain"
            || owned.prior != "scalar_normal"
            || owned.simulator != "linear_gaussian_design"
            || owned.likelihood != "known_noise_normal"
            || owned.random_seed_namespace != "gaussian_eig_v1_chacha20_box_muller"
            || owned.claim_status != "analytic_scalar_design_utility_only"
        {
            return Err(serde::de::Error::custom(
                "unexpected Gaussian EIG result identity",
            ));
        }
        Ok(Self {
            format: "marklab.gaussian_expected_information_gain",
            version: owned.version,
            candidate_id: owned.candidate_id,
            prior: "scalar_normal",
            simulator: "linear_gaussian_design",
            likelihood: "known_noise_normal",
            outer_samples: owned.outer_samples,
            inner_samples: owned.inner_samples,
            outer_values: owned.outer_values,
            estimate: owned.estimate,
            monte_carlo_se: owned.monte_carlo_se,
            analytic_eig: owned.analytic_eig,
            signed_bias_against_analytic: owned.signed_bias_against_analytic,
            absolute_bias_against_analytic: owned.absolute_bias_against_analytic,
            likelihood_evaluations: owned.likelihood_evaluations,
            random_seed_namespace: "gaussian_eig_v1_chacha20_box_muller",
            claim_status: "analytic_scalar_design_utility_only",
        })
    }
}

pub fn estimate_gaussian_expected_information_gain(
    spec: GaussianEigSpec,
) -> Result<GaussianEigResult, CausalError> {
    let work = validate(&spec)?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut outer_values = Vec::with_capacity(spec.outer_samples);
    for outer_index in 0..spec.outer_samples {
        let theta = sample_normal(spec.prior_mean, spec.prior_sd, &mut rng);
        let simulated_observation =
            spec.sensitivity * theta + sample_normal(0.0, spec.noise_sd, &mut rng);
        let log_numerator = log_likelihood(
            simulated_observation,
            theta,
            spec.sensitivity,
            spec.noise_sd,
        );
        let mut inner_logs = Vec::with_capacity(spec.inner_samples);
        for _ in 0..spec.inner_samples {
            let inner_theta = sample_normal(spec.prior_mean, spec.prior_sd, &mut rng);
            inner_logs.push(log_likelihood(
                simulated_observation,
                inner_theta,
                spec.sensitivity,
                spec.noise_sd,
            ));
        }
        let maximum = inner_logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let scaled_sum = inner_logs
            .iter()
            .map(|value| (value - maximum).exp())
            .sum::<f64>();
        let log_denominator = maximum + (scaled_sum / spec.inner_samples as f64).ln();
        let information_value = log_numerator - log_denominator;
        if !theta.is_finite()
            || !simulated_observation.is_finite()
            || !log_numerator.is_finite()
            || !log_denominator.is_finite()
            || !information_value.is_finite()
        {
            return Err(CausalError::Numerical(format!(
                "EIG outer value {outer_index} is not finite"
            )));
        }
        outer_values.push(GaussianEigOuterValue {
            outer_index,
            theta,
            simulated_observation,
            log_numerator,
            log_denominator,
            information_value,
        });
    }
    let estimate = outer_values
        .iter()
        .map(|value| value.information_value)
        .sum::<f64>()
        / spec.outer_samples as f64;
    let sample_variance = outer_values
        .iter()
        .map(|value| (value.information_value - estimate).powi(2))
        .sum::<f64>()
        / (spec.outer_samples - 1) as f64;
    let monte_carlo_se = (sample_variance / spec.outer_samples as f64).sqrt();
    let signal_variance = spec.sensitivity.powi(2) * spec.prior_sd.powi(2);
    let analytic_eig = 0.5 * (1.0 + signal_variance / spec.noise_sd.powi(2)).ln();
    let signed_bias = estimate - analytic_eig;
    if !estimate.is_finite() || !monte_carlo_se.is_finite() || !analytic_eig.is_finite() {
        return Err(CausalError::Numerical(
            "aggregate EIG result is not finite".into(),
        ));
    }
    Ok(GaussianEigResult {
        format: "marklab.gaussian_expected_information_gain",
        version: 1,
        candidate_id: spec.candidate_id,
        prior: "scalar_normal",
        simulator: "linear_gaussian_design",
        likelihood: "known_noise_normal",
        outer_samples: spec.outer_samples,
        inner_samples: spec.inner_samples,
        outer_values,
        estimate,
        monte_carlo_se,
        analytic_eig,
        signed_bias_against_analytic: signed_bias,
        absolute_bias_against_analytic: signed_bias.abs(),
        likelihood_evaluations: work,
        random_seed_namespace: "gaussian_eig_v1_chacha20_box_muller",
        claim_status: "analytic_scalar_design_utility_only",
    })
}

fn validate(spec: &GaussianEigSpec) -> Result<u64, CausalError> {
    if spec.candidate_id.trim().is_empty()
        || !spec.prior_mean.is_finite()
        || !spec.prior_sd.is_finite()
        || spec.prior_sd <= 0.0
        || !spec.sensitivity.is_finite()
        || spec.sensitivity == 0.0
        || !spec.noise_sd.is_finite()
        || spec.noise_sd <= 0.0
        || spec.outer_samples < 2
        || spec.outer_samples > MAXIMUM_OUTER_VALUES
        || spec.inner_samples == 0
    {
        return Err(CausalError::Invalid(
            "Gaussian EIG parameters violate identity/finite/positive/sample requirements".into(),
        ));
    }
    let work = u64::try_from(spec.outer_samples)
        .ok()
        .and_then(|outer| {
            u64::try_from(spec.inner_samples + 1)
                .ok()
                .and_then(|inner| outer.checked_mul(inner))
        })
        .ok_or_else(|| CausalError::Resource("EIG likelihood work overflowed".into()))?;
    if work > spec.maximum_likelihood_evaluations || work > MAXIMUM_LIKELIHOOD_EVALUATIONS {
        return Err(CausalError::Resource(format!(
            "EIG likelihood evaluations {work} exceed caller or built-in maximum"
        )));
    }
    Ok(work)
}

fn sample_normal(mean: f64, sd: f64, rng: &mut ChaCha20Rng) -> f64 {
    let first_uniform = 1.0 - rng.gen::<f64>();
    let second_uniform = rng.gen::<f64>();
    let standard =
        (-2.0 * first_uniform.ln()).sqrt() * (2.0 * std::f64::consts::PI * second_uniform).cos();
    mean + sd * standard
}

fn log_likelihood(observation: f64, theta: f64, sensitivity: f64, noise_sd: f64) -> f64 {
    let standardized = (observation - sensitivity * theta) / noise_sd;
    -noise_sd.ln() - 0.5 * (2.0 * std::f64::consts::PI).ln() - 0.5 * standardized * standardized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_sensitivity_is_rejected_as_an_uninformative_declared_candidate() {
        let error = estimate_gaussian_expected_information_gain(GaussianEigSpec {
            candidate_id: "zero".into(),
            prior_mean: 0.0,
            prior_sd: 1.0,
            sensitivity: 0.0,
            noise_sd: 1.0,
            outer_samples: 2,
            inner_samples: 2,
            seed: 1,
            maximum_likelihood_evaluations: 6,
        })
        .unwrap_err();
        assert!(error.to_string().contains("requirements"));
    }
}

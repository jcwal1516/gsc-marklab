use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::BayesError;

pub const WORKER_REQUEST_FORMAT: &str = "marklab.pymc_worker_request";
pub const WORKER_REQUEST_VERSION: u32 = 1;
pub const MODEL_FORMAT: &str = "marklab.bayesian_model_ir";
pub const MODEL_VERSION: u32 = 1;
pub const PYMC_VERSION: &str = "6.3.0";

#[derive(Clone, Debug, Serialize)]
pub struct NormalMeanSpec {
    pub prior_mean: f64,
    pub prior_sd: f64,
    pub known_sigma: f64,
    pub observations: Vec<f64>,
}

impl NormalMeanSpec {
    pub fn validate(&self) -> Result<(), BayesError> {
        if self.observations.is_empty() {
            return Err(BayesError::InvalidSpec(
                "at least one observation is required".into(),
            ));
        }
        if self.observations.len() > 100_000 {
            return Err(BayesError::InvalidSpec(
                "observation count exceeds 100000".into(),
            ));
        }
        if !self.prior_mean.is_finite() {
            return Err(BayesError::InvalidSpec("prior mean must be finite".into()));
        }
        if !self.prior_sd.is_finite() || self.prior_sd <= 0.0 {
            return Err(BayesError::InvalidSpec(
                "prior standard deviation must be finite and positive".into(),
            ));
        }
        if !self.known_sigma.is_finite() || self.known_sigma <= 0.0 {
            return Err(BayesError::InvalidSpec(
                "known observation standard deviation must be finite and positive".into(),
            ));
        }
        if self.observations.iter().any(|value| !value.is_finite()) {
            return Err(BayesError::InvalidSpec(
                "observations must all be finite".into(),
            ));
        }
        Ok(())
    }

    pub fn model_ir(&self) -> NormalMeanModelIr {
        NormalMeanModelIr {
            format: MODEL_FORMAT,
            version: MODEL_VERSION,
            family: "normal_mean_known_sigma",
            parameter: ParameterIr {
                name: "mu",
                support: "real",
                interpretation: "population_mean",
            },
            prior: NormalPriorIr {
                family: "normal",
                mean: self.prior_mean,
                sd: self.prior_sd,
                rationale: "user_supplied",
            },
            likelihood: NormalLikelihoodIr {
                family: "normal_known_sigma",
                known_sigma: self.known_sigma,
            },
            observation_unit: "scalar_observation",
            generated_quantities: ["posterior_predictive_observation_mean"],
            backend_capability: "nuts",
            maturity: "experimental",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalMeanModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub parameter: ParameterIr,
    pub prior: NormalPriorIr,
    pub likelihood: NormalLikelihoodIr,
    pub observation_unit: &'static str,
    pub generated_quantities: [&'static str; 1],
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParameterIr {
    pub name: &'static str,
    pub support: &'static str,
    pub interpretation: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalPriorIr {
    pub family: &'static str,
    pub mean: f64,
    pub sd: f64,
    pub rationale: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalLikelihoodIr {
    pub family: &'static str,
    pub known_sigma: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackendContract {
    pub name: &'static str,
    pub version: &'static str,
    pub python_version: &'static str,
    pub environment_lock_sha256: String,
    pub worker_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NutsSamplingSpec {
    pub chains: u32,
    pub tune_per_chain: u32,
    pub draws_per_chain: u32,
    pub target_accept: f64,
    pub seed: u64,
}

impl NutsSamplingSpec {
    pub fn validate(&self) -> Result<(), BayesError> {
        if !(2..=8).contains(&self.chains) {
            return Err(BayesError::InvalidSpec(
                "NUTS requires between 2 and 8 chains".into(),
            ));
        }
        if !(100..=100_000).contains(&self.tune_per_chain) {
            return Err(BayesError::InvalidSpec(
                "tune iterations per chain must be between 100 and 100000".into(),
            ));
        }
        if !(100..=100_000).contains(&self.draws_per_chain) {
            return Err(BayesError::InvalidSpec(
                "posterior draws per chain must be between 100 and 100000".into(),
            ));
        }
        if !self.target_accept.is_finite() || self.target_accept < 0.5 || self.target_accept >= 1.0
        {
            return Err(BayesError::InvalidSpec(
                "target acceptance must be finite and in [0.5, 1)".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkerResourceLimits {
    pub maximum_observations: u32,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticPolicy {
    pub prior_predictive_draws: u32,
    pub maximum_r_hat: f64,
    pub minimum_bulk_ess: f64,
    pub minimum_tail_ess: f64,
    pub minimum_ebfmi: f64,
    pub maximum_divergences: u64,
    pub maximum_tree_depth_hits: u64,
    pub maximum_tree_depth: u32,
}

impl Default for DiagnosticPolicy {
    fn default() -> Self {
        Self {
            prior_predictive_draws: 500,
            maximum_r_hat: 1.01,
            minimum_bulk_ess: 400.0,
            minimum_tail_ess: 400.0,
            minimum_ebfmi: 0.3,
            maximum_divergences: 0,
            maximum_tree_depth_hits: 0,
            maximum_tree_depth: 10,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalMeanWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: NormalMeanModelIr,
    pub observations: Vec<f64>,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl NormalMeanWorkerRequest {
    pub fn new(
        spec: &NormalMeanSpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        spec.validate()?;
        sampling.validate()?;
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let total_iterations = u64::from(sampling.chains)
            .checked_mul(u64::from(
                sampling.tune_per_chain + sampling.draws_per_chain,
            ))
            .ok_or_else(|| BayesError::InvalidSpec("total NUTS iterations overflow".into()))?;
        let maximum_total_iterations = 800_000;
        if total_iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(format!(
                "requested {total_iterations} NUTS iterations exceed the {maximum_total_iterations} limit"
            )));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "pymc",
                version: PYMC_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: spec.model_ir(),
            observations: spec.observations.clone(),
            sampling,
            resources: WorkerResourceLimits {
                maximum_observations: 100_000,
                maximum_total_iterations,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_mean_spec_rejects_nonfinite_and_nonpositive_inputs() {
        let invalid = NormalMeanSpec {
            prior_mean: 0.0,
            prior_sd: 0.0,
            known_sigma: 1.0,
            observations: vec![1.0],
        };
        assert!(matches!(
            invalid.validate(),
            Err(BayesError::InvalidSpec(_))
        ));

        let invalid = NormalMeanSpec {
            prior_mean: 0.0,
            prior_sd: 1.0,
            known_sigma: 1.0,
            observations: vec![f64::NAN],
        };
        assert!(matches!(
            invalid.validate(),
            Err(BayesError::InvalidSpec(_))
        ));
    }

    #[test]
    fn normal_mean_ir_exposes_all_scientific_defaults() {
        let spec = NormalMeanSpec {
            prior_mean: 0.0,
            prior_sd: 1.0,
            known_sigma: 2.0,
            observations: vec![1.0, 2.0],
        };
        let value = serde_json::to_value(spec.model_ir()).expect("model IR");
        assert_eq!(value["family"], "normal_mean_known_sigma");
        assert_eq!(value["likelihood"]["known_sigma"], 2.0);
        assert_eq!(value["backend_capability"], "nuts");
        assert_eq!(value["maturity"], "experimental");
    }
}

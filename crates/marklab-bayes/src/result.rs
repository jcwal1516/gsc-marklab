use serde::{Deserialize, Serialize};

use crate::{model::PYMC_VERSION, BayesError, NormalMeanWorkerRequest};

const WORKER_RESULT_FORMAT: &str = "marklab.pymc_worker_result";
const WORKER_RESULT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FitState {
    Complete,
    Nonconverged,
    ApproximateOnly,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerBackend {
    pub name: String,
    pub version: String,
    pub python_version: String,
    pub environment_lock_sha256: String,
    pub worker_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingSummary {
    pub chains: u32,
    pub tune_per_chain: u32,
    pub draws_per_chain: u32,
    pub completed_draws: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalMeanPosterior {
    pub parameter: String,
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalMeanDiagnostics {
    pub prior_predictive_finite: bool,
    pub posterior_finite: bool,
    pub r_hat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
    pub mcse_mean: f64,
    pub mcse_sd: f64,
    pub minimum_ebfmi: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
    pub constraints_valid: bool,
    pub identifiability_checks_passed: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalMeanPosteriorPredictive {
    pub observed_mean: f64,
    pub replicated_mean_mean: f64,
    pub replicated_mean_sd: f64,
    pub probability_replicated_mean_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: NormalMeanPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: NormalMeanPosteriorPredictive,
}

impl WorkerResult {
    pub fn validate(
        &self,
        request: &NormalMeanWorkerRequest,
        expected_request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly {
            return Err(BayesError::WorkerContract(
                "normal-mean NUTS cannot return approximate-only state".into(),
            ));
        }
        if self.format != WORKER_RESULT_FORMAT || self.version != WORKER_RESULT_VERSION {
            return Err(BayesError::WorkerContract(format!(
                "unsupported worker result {}/{}",
                self.format, self.version
            )));
        }
        if self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
        {
            return Err(BayesError::WorkerContract(
                "worker backend identity does not match the request".into(),
            ));
        }
        if self.request_sha256 != expected_request_sha256 {
            return Err(BayesError::WorkerContract(
                "worker result is not bound to the exact request bytes".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
        {
            return Err(BayesError::WorkerContract(
                "worker sampling counts do not match the request".into(),
            ));
        }
        if self.posterior.parameter != "mu"
            || !all_finite(&[
                self.posterior.mean,
                self.posterior.sd,
                self.posterior.interval_lower,
                self.posterior.interval_upper,
                self.diagnostics.r_hat,
                self.diagnostics.ess_bulk,
                self.diagnostics.ess_tail,
                self.diagnostics.mcse_mean,
                self.diagnostics.mcse_sd,
                self.diagnostics.minimum_ebfmi,
                self.posterior_predictive.observed_mean,
                self.posterior_predictive.replicated_mean_mean,
                self.posterior_predictive.replicated_mean_sd,
                self.posterior_predictive
                    .probability_replicated_mean_at_least_observed,
            ])
            || self.posterior.sd <= 0.0
            || self.posterior.interval_lower > self.posterior.interval_upper
            || self.posterior_predictive.replicated_mean_sd < 0.0
            || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
            || !(0.0..=1.0).contains(
                &self
                    .posterior_predictive
                    .probability_replicated_mean_at_least_observed,
            )
        {
            return Err(BayesError::WorkerContract(
                "worker returned an invalid or non-finite posterior result".into(),
            ));
        }
        let observed_mean =
            request.observations.iter().sum::<f64>() / request.observations.len() as f64;
        let tolerance = 1e-12 * observed_mean.abs().max(1.0);
        if (self.posterior_predictive.observed_mean - observed_mean).abs() > tolerance {
            return Err(BayesError::WorkerContract(
                "posterior predictive result changed the observed data".into(),
            ));
        }
        let diagnostics_pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= request.diagnostic_policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= request.diagnostic_policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= request.diagnostic_policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= request.diagnostic_policy.minimum_ebfmi
            && self.diagnostics.divergences <= request.diagnostic_policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits
                <= request.diagnostic_policy.maximum_tree_depth_hits;
        if (self.fit_state == FitState::Complete) != diagnostics_pass {
            return Err(BayesError::WorkerContract(
                "fit state disagrees with the declared diagnostic policy".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: NormalMeanWorkerRequest,
        input: NormalMeanInputIdentity,
    ) -> NormalMeanFit {
        NormalMeanFit {
            format: "marklab.bayesian_fit",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            fit_state: self.fit_state,
            claim_status: match self.fit_state {
                FitState::Complete => "experimental",
                FitState::Nonconverged => "diagnostic_only_nonconverged",
                FitState::ApproximateOnly => "experimental_approximate_only",
            },
            sampling: self.sampling,
            posterior: self.posterior,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn all_finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalMeanInputIdentity {
    pub path: String,
    pub observation_count: usize,
    pub observations_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct NormalMeanFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: crate::NormalMeanModelIr,
    pub input: NormalMeanInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: NormalMeanPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: NormalMeanPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NormalMeanSpec, NutsSamplingSpec};

    fn request() -> NormalMeanWorkerRequest {
        NormalMeanWorkerRequest::new(
            &NormalMeanSpec {
                prior_mean: 0.0,
                prior_sd: 1.0,
                known_sigma: 1.0,
                observations: vec![1.0, 2.0, 3.0, 4.0],
            },
            NutsSamplingSpec {
                chains: 2,
                tune_per_chain: 500,
                draws_per_chain: 1_000,
                target_accept: 0.9,
                seed: 7,
            },
            "lock".into(),
            "worker".into(),
            180,
        )
        .expect("request")
    }

    fn response(fit_state: &str, ess: f64) -> serde_json::Value {
        serde_json::json!({
            "format": "marklab.pymc_worker_result",
            "version": 1,
            "backend": {
                "name": "pymc",
                "version": "6.3.0",
                "python_version": "3.12",
                "environment_lock_sha256": "lock",
                "worker_sha256": "worker"
            },
            "request_sha256": "request",
            "fit_state": fit_state,
            "sampling": {
                "chains": 2,
                "tune_per_chain": 500,
                "draws_per_chain": 1000,
                "completed_draws": 2000
            },
            "posterior": {
                "parameter": "mu",
                "mean": 2.0,
                "sd": 0.45,
                "interval_lower": 1.1,
                "interval_upper": 2.9
            },
            "diagnostics": {
                "prior_predictive_finite": true,
                "posterior_finite": true,
                "r_hat": 1.0,
                "ess_bulk": ess,
                "ess_tail": ess,
                "mcse_mean": 0.01,
                "mcse_sd": 0.01,
                "minimum_ebfmi": 0.8,
                "divergences": 0,
                "max_tree_depth_hits": 0,
                "constraints_valid": true,
                "identifiability_checks_passed": true
            },
            "posterior_predictive": {
                "observed_mean": 2.5,
                "replicated_mean_mean": 2.0,
                "replicated_mean_sd": 0.6,
                "probability_replicated_mean_at_least_observed": 0.2
            }
        })
    }

    #[test]
    fn response_rejects_unknown_fields() {
        let mut value = response("complete", 500.0);
        value["unexpected"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<WorkerResult>(value).is_err());
    }

    #[test]
    fn complete_state_must_match_diagnostic_policy() {
        let result: WorkerResult =
            serde_json::from_value(response("complete", 100.0)).expect("response schema");
        assert!(matches!(
            result.validate(&request(), "request"),
            Err(BayesError::WorkerContract(_))
        ));

        let result: WorkerResult =
            serde_json::from_value(response("nonconverged", 100.0)).expect("response schema");
        result
            .validate(&request(), "request")
            .expect("typed nonconverged response");
    }
}

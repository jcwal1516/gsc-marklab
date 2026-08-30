use crate::validation::all_finite as finite;

use serde::{Deserialize, Serialize};

use crate::{
    model::{BackendContract, PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    BayesError, FitState, NormalMeanModelIr, NormalMeanPosteriorPredictive, NormalMeanSpec,
    SarScalarSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct SmcSamplingSpec {
    pub particles: u32,
    pub chains: u32,
    pub ess_target: f64,
    pub correlation_threshold: f64,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SmcResourceLimits {
    pub maximum_observations: u32,
    pub maximum_total_particles: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalMeanSmcWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: NormalMeanModelIr,
    pub observations: Vec<f64>,
    pub sampling: SmcSamplingSpec,
    pub resources: SmcResourceLimits,
}

impl NormalMeanSmcWorkerRequest {
    pub fn new(
        spec: NormalMeanSpec,
        sampling: SmcSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        spec.validate()?;
        if !(100..=10_000).contains(&sampling.particles)
            || !(2..=4).contains(&sampling.chains)
            || !sampling.ess_target.is_finite()
            || sampling.ess_target <= 0.0
            || sampling.ess_target >= 1.0
            || !sampling.correlation_threshold.is_finite()
            || sampling.correlation_threshold <= 0.0
            || sampling.correlation_threshold >= 1.0
        {
            return Err(BayesError::InvalidSpec(
                "SMC requires 100-10000 particles, 2-4 chains, and targets in (0,1)".into(),
            ));
        }
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "SMC worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let total_particles = u64::from(sampling.particles) * u64::from(sampling.chains);
        if total_particles > 40_000 {
            return Err(BayesError::InvalidSpec(
                "SMC total particle count exceeds 40000".into(),
            ));
        }
        let mut model = spec.model_ir();
        model.backend_capability = "annealed_smc";
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
            model,
            observations: spec.observations,
            sampling,
            resources: SmcResourceLimits {
                maximum_observations: 100_000,
                maximum_total_particles: 40_000,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SmcStage {
    pub beta: f64,
    pub ess: f64,
    pub acceptance_rate: f64,
    pub ancestor_indexes: Vec<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SmcChain {
    pub chain: u32,
    pub log_marginal_likelihood: f64,
    pub stages: Vec<SmcStage>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SmcEvidence {
    pub chain_log_marginal_likelihoods: Vec<f64>,
    pub mean: f64,
    pub sd: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SmcDiagnostics {
    pub prior_particles_finite: bool,
    pub posterior_particles_finite: bool,
    pub posterior_predictive_finite: bool,
    pub all_final_beta_one: bool,
    pub ancestry_valid: bool,
    pub evidence_finite: bool,
    pub minimum_ess: f64,
    pub minimum_ess_ratio: f64,
    pub minimum_acceptance_rate: f64,
    pub maximum_acceptance_rate: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalMeanSmcWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub posterior: SarScalarSummary,
    pub evidence: SmcEvidence,
    pub chains: Vec<SmcChain>,
    pub diagnostics: SmcDiagnostics,
    pub posterior_predictive: NormalMeanPosteriorPredictive,
}

impl NormalMeanSmcWorkerResult {
    pub fn validate(
        &self,
        request: &NormalMeanSmcWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly
            || self.format != "marklab.pymc_normal_mean_smc_worker_result"
            || self.version != 1
            || self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
        {
            return Err(BayesError::WorkerContract(
                "normal-mean SMC result identity mismatch".into(),
            ));
        }
        validate_scalar(&self.posterior)?;
        if self.chains.len() != request.sampling.chains as usize
            || self.evidence.chain_log_marginal_likelihoods.len()
                != request.sampling.chains as usize
            || !finite(&[
                self.evidence.mean,
                self.evidence.sd,
                self.diagnostics.minimum_ess,
                self.diagnostics.minimum_ess_ratio,
                self.diagnostics.minimum_acceptance_rate,
                self.diagnostics.maximum_acceptance_rate,
            ])
            || self.evidence.sd < 0.0
            || self.diagnostics.minimum_ess <= 0.0
            || !(0.0..=1.0).contains(&self.diagnostics.minimum_ess_ratio)
            || !(0.0..=1.0).contains(&self.diagnostics.minimum_acceptance_rate)
            || !(0.0..=1.0).contains(&self.diagnostics.maximum_acceptance_rate)
            || self.diagnostics.minimum_acceptance_rate > self.diagnostics.maximum_acceptance_rate
        {
            return Err(BayesError::WorkerContract(
                "normal-mean SMC summary counts or diagnostics are invalid".into(),
            ));
        }
        for (chain_index, chain) in self.chains.iter().enumerate() {
            if chain.chain != chain_index as u32
                || chain.stages.is_empty()
                || !chain.log_marginal_likelihood.is_finite()
                || chain.log_marginal_likelihood.to_bits()
                    != self.evidence.chain_log_marginal_likelihoods[chain_index].to_bits()
            {
                return Err(BayesError::WorkerContract(
                    "normal-mean SMC chain identity or evidence mismatch".into(),
                ));
            }
            let mut previous_beta = 0.0;
            for stage in &chain.stages {
                if !stage.beta.is_finite()
                    || stage.beta <= previous_beta
                    || stage.beta > 1.0
                    || !stage.ess.is_finite()
                    || stage.ess <= 0.0
                    || stage.ess > f64::from(request.sampling.particles)
                    || !stage.acceptance_rate.is_finite()
                    || !(0.0..=1.0).contains(&stage.acceptance_rate)
                    || stage.ancestor_indexes.len() != request.sampling.particles as usize
                    || stage
                        .ancestor_indexes
                        .iter()
                        .any(|&index| index >= request.sampling.particles)
                {
                    return Err(BayesError::WorkerContract(
                        "normal-mean SMC stage or ancestry is invalid".into(),
                    ));
                }
                previous_beta = stage.beta;
            }
            if previous_beta.to_bits() != 1.0_f64.to_bits() {
                return Err(BayesError::WorkerContract(
                    "normal-mean SMC chain did not reach beta one".into(),
                ));
            }
        }
        let observed_mean =
            request.observations.iter().sum::<f64>() / request.observations.len() as f64;
        if !finite(&[
            self.posterior_predictive.observed_mean,
            self.posterior_predictive.replicated_mean_mean,
            self.posterior_predictive.replicated_mean_sd,
            self.posterior_predictive
                .probability_replicated_mean_at_least_observed,
        ]) || (self.posterior_predictive.observed_mean - observed_mean).abs()
            > 1e-12 * observed_mean.abs().max(1.0)
            || self.posterior_predictive.replicated_mean_sd < 0.0
            || !(0.0..=1.0).contains(
                &self
                    .posterior_predictive
                    .probability_replicated_mean_at_least_observed,
            )
        {
            return Err(BayesError::WorkerContract(
                "normal-mean SMC posterior predictive is invalid".into(),
            ));
        }
        let pass = self.diagnostics.prior_particles_finite
            && self.diagnostics.posterior_particles_finite
            && self.diagnostics.posterior_predictive_finite
            && self.diagnostics.all_final_beta_one
            && self.diagnostics.ancestry_valid
            && self.diagnostics.evidence_finite;
        if (self.fit_state == FitState::Complete) != pass {
            return Err(BayesError::WorkerContract(
                "normal-mean SMC fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: NormalMeanSmcWorkerRequest,
        input: NormalMeanSmcInputIdentity,
    ) -> NormalMeanSmcFit {
        NormalMeanSmcFit {
            format: "marklab.bayesian_normal_mean_smc",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            sampling: request.sampling,
            fit_state: self.fit_state,
            claim_status: match self.fit_state {
                FitState::Complete => "experimental",
                FitState::Nonconverged => "diagnostic_only_nonconverged",
                FitState::ApproximateOnly => "experimental_approximate_only",
            },
            posterior: self.posterior,
            evidence: self.evidence,
            chains: self.chains,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_scalar(summary: &SarScalarSummary) -> Result<(), BayesError> {
    if !finite(&[
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]) || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
    {
        return Err(BayesError::WorkerContract(
            "invalid normal-mean SMC posterior".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct NormalMeanSmcInputIdentity {
    pub path: String,
    pub observation_count: usize,
    pub observations_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct NormalMeanSmcFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: NormalMeanModelIr,
    pub input: NormalMeanSmcInputIdentity,
    pub sampling: SmcSamplingSpec,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub posterior: SarScalarSummary,
    pub evidence: SmcEvidence,
    pub chains: Vec<SmcChain>,
    pub diagnostics: SmcDiagnostics,
    pub posterior_predictive: NormalMeanPosteriorPredictive,
    pub request_sha256: String,
}

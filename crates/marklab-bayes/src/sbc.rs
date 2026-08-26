use serde::{Deserialize, Serialize};

use crate::{
    model::{BackendContract, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    BayesError, FitState, WorkerBackend,
};

const SBC_BACKEND_VERSION: &str = "numpy-2.4.6+scipy-1.18.1";

#[derive(Clone, Debug)]
pub struct NormalMeanSbcSpec {
    pub prior_mean: f64,
    pub prior_sd: f64,
    pub known_sigma: f64,
    pub observations_per_replicate: u32,
    pub replicates: u32,
    pub posterior_draws: u32,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalMeanSbcModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub parameter: &'static str,
    pub prior: &'static str,
    pub prior_mean: f64,
    pub prior_sd: f64,
    pub likelihood: &'static str,
    pub known_sigma: f64,
    pub inference_algorithm: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct SbcExecutionSpec {
    pub observations_per_replicate: u32,
    pub replicates: u32,
    pub posterior_draws: u32,
    pub interval_probability: f64,
    pub rank_histogram_bins: u32,
    pub uniformity_p_value_minimum: f64,
    pub envelope_alpha: f64,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SbcResourceLimits {
    pub maximum_simulated_values: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalMeanSbcWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: NormalMeanSbcModelIr,
    pub sbc: SbcExecutionSpec,
    pub resources: SbcResourceLimits,
}

impl NormalMeanSbcWorkerRequest {
    pub fn new(
        spec: NormalMeanSbcSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if !spec.prior_mean.is_finite()
            || !spec.prior_sd.is_finite()
            || spec.prior_sd <= 0.0
            || !spec.known_sigma.is_finite()
            || spec.known_sigma <= 0.0
            || !(1..=1_000).contains(&spec.observations_per_replicate)
            || !(20..=5_000).contains(&spec.replicates)
            || !(20..=5_000).contains(&spec.posterior_draws)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "normal-mean SBC model or execution controls are invalid".into(),
            ));
        }
        let simulated_values = u64::from(spec.replicates)
            .checked_mul(u64::from(
                spec.observations_per_replicate + spec.posterior_draws,
            ))
            .ok_or_else(|| BayesError::InvalidSpec("SBC work bound overflows".into()))?;
        let maximum_simulated_values = 2_000_000;
        if simulated_values > maximum_simulated_values {
            return Err(BayesError::InvalidSpec(
                "normal-mean SBC exceeds 2000000 simulated values".into(),
            ));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "numpy_scipy",
                version: SBC_BACKEND_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: NormalMeanSbcModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "normal_mean_known_sigma",
                parameter: "mu",
                prior: "normal",
                prior_mean: spec.prior_mean,
                prior_sd: spec.prior_sd,
                likelihood: "normal_known_sigma",
                known_sigma: spec.known_sigma,
                inference_algorithm: "exact_conjugate_independent_posterior_sampler",
                backend_capability: "simulation_based_calibration",
                maturity: "experimental_calibration",
            },
            sbc: SbcExecutionSpec {
                observations_per_replicate: spec.observations_per_replicate,
                replicates: spec.replicates,
                posterior_draws: spec.posterior_draws,
                interval_probability: 0.95,
                rank_histogram_bins: 20_u32.min(spec.posterior_draws + 1),
                uniformity_p_value_minimum: 0.01,
                envelope_alpha: 0.01,
                seed: spec.seed,
            },
            resources: SbcResourceLimits {
                maximum_simulated_values,
                maximum_output_bytes: 4 * 1_048_576,
                timeout_seconds,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SbcReplicate {
    pub replicate: u32,
    pub true_value: f64,
    pub posterior_mean: f64,
    pub posterior_sd: f64,
    pub rank: u32,
    pub interval_lower: f64,
    pub interval_upper: f64,
    pub covered: bool,
    pub z_score: f64,
    pub shrinkage: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SbcFailure {
    pub replicate: u32,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SbcDiagnostics {
    pub rank_histogram: Vec<u32>,
    pub rank_uniformity_chi_square: f64,
    pub rank_uniformity_p_value: f64,
    pub maximum_ecdf_deviation: f64,
    pub ecdf_envelope: f64,
    pub coverage_95: f64,
    pub coverage_standardized_error: f64,
    pub z_score_mean: f64,
    pub z_score_sd: f64,
    pub mean_shrinkage: f64,
    pub failure_rate: f64,
    pub posterior_draws_exchangeable: bool,
    pub autocorrelation_correction: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalMeanSbcWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub replicates: Vec<SbcReplicate>,
    pub failures: Vec<SbcFailure>,
    pub diagnostics: SbcDiagnostics,
}

impl NormalMeanSbcWorkerResult {
    pub fn validate(
        &self,
        request: &NormalMeanSbcWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.numpy_scipy_normal_mean_sbc_worker_result"
            || self.version != 1
            || self.backend.name != "numpy_scipy"
            || self.backend.version != SBC_BACKEND_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.fit_state == FitState::ApproximateOnly
            || self.replicates.len() + self.failures.len() != request.sbc.replicates as usize
            || self.diagnostics.rank_histogram.len() != request.sbc.rank_histogram_bins as usize
        {
            return Err(BayesError::WorkerContract(
                "normal-mean SBC result identity or dimensions mismatch".into(),
            ));
        }
        let mut seen = vec![false; request.sbc.replicates as usize];
        let mut histogram = vec![0_u32; request.sbc.rank_histogram_bins as usize];
        let mut covered = 0_u32;
        let mut ranks = Vec::new();
        let mut z_scores = Vec::new();
        let mut shrinkages = Vec::new();
        for replicate in &self.replicates {
            let index = replicate.replicate as usize;
            if index >= seen.len()
                || seen[index]
                || replicate.rank > request.sbc.posterior_draws
                || !finite(&[
                    replicate.true_value,
                    replicate.posterior_mean,
                    replicate.posterior_sd,
                    replicate.interval_lower,
                    replicate.interval_upper,
                    replicate.z_score,
                    replicate.shrinkage,
                ])
                || replicate.posterior_sd <= 0.0
                || replicate.interval_lower > replicate.interval_upper
                || replicate.covered
                    != (replicate.true_value >= replicate.interval_lower
                        && replicate.true_value <= replicate.interval_upper)
                || !approximately_equal(
                    replicate.z_score,
                    (replicate.true_value - replicate.posterior_mean) / replicate.posterior_sd,
                )
                || !approximately_equal(
                    replicate.shrinkage,
                    1.0 - replicate.posterior_sd.powi(2) / request.model.prior_sd.powi(2),
                )
            {
                return Err(BayesError::WorkerContract(
                    "normal-mean SBC replicate is invalid".into(),
                ));
            }
            seen[index] = true;
            let bin = (replicate.rank as usize * histogram.len())
                / (request.sbc.posterior_draws as usize + 1);
            let maximum_bin = histogram.len() - 1;
            histogram[bin.min(maximum_bin)] += 1;
            covered += u32::from(replicate.covered);
            ranks.push(replicate.rank);
            z_scores.push(replicate.z_score);
            shrinkages.push(replicate.shrinkage);
        }
        for failure in &self.failures {
            let index = failure.replicate as usize;
            if index >= seen.len() || seen[index] || failure.reason.is_empty() {
                return Err(BayesError::WorkerContract(
                    "normal-mean SBC failure entry is invalid".into(),
                ));
            }
            seen[index] = true;
        }
        let success_count = self.replicates.len();
        if success_count < 2 || !seen.into_iter().all(|value| value) {
            return Err(BayesError::WorkerContract(
                "normal-mean SBC lacks complete replicate disposition".into(),
            ));
        }
        let coverage = f64::from(covered) / success_count as f64;
        let coverage_standardized_error = (coverage - request.sbc.interval_probability)
            / (request.sbc.interval_probability * (1.0 - request.sbc.interval_probability)
                / success_count as f64)
                .sqrt();
        let expected_bin_count = success_count as f64 / histogram.len() as f64;
        let rank_uniformity_chi_square = histogram
            .iter()
            .map(|count| (f64::from(*count) - expected_bin_count).powi(2) / expected_bin_count)
            .sum::<f64>();
        ranks.sort_unstable();
        let maximum_ecdf_deviation = ranks
            .iter()
            .enumerate()
            .map(|(index, rank)| {
                let empirical = (index + 1) as f64 / success_count as f64;
                let expected = f64::from(*rank + 1) / f64::from(request.sbc.posterior_draws + 1);
                (empirical - expected).abs()
            })
            .fold(0.0_f64, f64::max);
        let ecdf_envelope =
            (f64::ln(2.0 / request.sbc.envelope_alpha) / (2.0 * success_count as f64)).sqrt();
        let z_mean = mean(&z_scores);
        let z_sd = sample_sd(&z_scores, z_mean);
        let shrinkage = mean(&shrinkages);
        let failure_rate = self.failures.len() as f64 / request.sbc.replicates as f64;
        if histogram != self.diagnostics.rank_histogram
            || !finite(&[
                self.diagnostics.rank_uniformity_chi_square,
                self.diagnostics.rank_uniformity_p_value,
                self.diagnostics.maximum_ecdf_deviation,
                self.diagnostics.ecdf_envelope,
                self.diagnostics.coverage_95,
                self.diagnostics.coverage_standardized_error,
                self.diagnostics.z_score_mean,
                self.diagnostics.z_score_sd,
                self.diagnostics.mean_shrinkage,
                self.diagnostics.failure_rate,
            ])
            || !(0.0..=1.0).contains(&self.diagnostics.rank_uniformity_p_value)
            || !approximately_equal(
                self.diagnostics.rank_uniformity_chi_square,
                rank_uniformity_chi_square,
            )
            || !approximately_equal(
                self.diagnostics.maximum_ecdf_deviation,
                maximum_ecdf_deviation,
            )
            || !approximately_equal(self.diagnostics.ecdf_envelope, ecdf_envelope)
            || !approximately_equal(self.diagnostics.coverage_95, coverage)
            || !approximately_equal(
                self.diagnostics.coverage_standardized_error,
                coverage_standardized_error,
            )
            || !approximately_equal(self.diagnostics.z_score_mean, z_mean)
            || !approximately_equal(self.diagnostics.z_score_sd, z_sd)
            || !approximately_equal(self.diagnostics.mean_shrinkage, shrinkage)
            || !approximately_equal(self.diagnostics.failure_rate, failure_rate)
            || !self.diagnostics.posterior_draws_exchangeable
            || self.diagnostics.autocorrelation_correction
                != "not_required_independent_conjugate_draws"
        {
            return Err(BayesError::WorkerContract(
                "normal-mean SBC diagnostics are invalid".into(),
            ));
        }
        let valid = self.failures.is_empty()
            && self.diagnostics.rank_uniformity_p_value >= request.sbc.uniformity_p_value_minimum
            && self.diagnostics.maximum_ecdf_deviation <= self.diagnostics.ecdf_envelope
            && self.diagnostics.coverage_standardized_error.abs() <= 3.0
            && self.diagnostics.z_score_mean.abs() <= 3.0 / (request.sbc.replicates as f64).sqrt()
            && (0.8..=1.2).contains(&self.diagnostics.z_score_sd);
        if (self.fit_state == FitState::Complete) != valid {
            return Err(BayesError::WorkerContract(
                "normal-mean SBC fit state disagrees with calibration gates".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(self, request: NormalMeanSbcWorkerRequest) -> NormalMeanSbcResult {
        NormalMeanSbcResult {
            format: "marklab.bayesian_normal_mean_sbc",
            version: 1,
            backend: self.backend,
            model: request.model,
            sbc: request.sbc,
            fit_state: self.fit_state,
            replicates: self.replicates,
            failures: self.failures,
            diagnostics: self.diagnostics,
            claim_status: "experimental_calibration_procedure",
            request_sha256: self.request_sha256,
        }
    }
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-10 * left.abs().max(right.abs()).max(1.0)
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn sample_sd(values: &[f64], mean: f64) -> f64 {
    (values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64)
        .sqrt()
}

#[derive(Debug, Serialize)]
pub struct NormalMeanSbcResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: NormalMeanSbcModelIr,
    pub sbc: SbcExecutionSpec,
    pub fit_state: FitState,
    pub replicates: Vec<SbcReplicate>,
    pub failures: Vec<SbcFailure>,
    pub diagnostics: SbcDiagnostics,
    pub claim_status: &'static str,
    pub request_sha256: String,
}

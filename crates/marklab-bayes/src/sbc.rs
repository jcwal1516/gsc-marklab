use crate::is_worker_timeout;
use crate::validation::all_finite as finite;
use marklab_numerics::relative_eq;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::backend_versions::NUMPY_SCIPY_BACKEND;
use crate::backend_versions::PYTHON_VERSION;
use crate::ClaimStatus;
use crate::{
    model::{BackendContract, DiagnosticPolicy, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    BayesError, FitState, HierarchicalSbcFailure, WorkerBackend,
};
use marklab_core::backend::BackendCapability;
use marklab_core::backend::ModelMaturity;

const RELATIVE_TOLERANCE: f64 = 1e-10;
const Z_975: f64 = 1.959_963_984_540_054;
const EXACT_MULTINOMIAL_MAXIMUM_COUNT: u64 = 50;
const EXACT_MULTINOMIAL_MAXIMUM_BINS: u64 = 20;
// The default 10- and 20-bin policies visit fewer than four million simulated ranks. Custom
// policies remain bounded by this budget times their declared completed-replicate count.
const MONTE_CARLO_REPLICATES: u64 = 20_000;
const MONTE_CARLO_SEED: u64 = 0x6a09_e667_f3bc_c909;

pub(crate) fn rank_uniformity(histogram: &[u32], posterior_draws: u32) -> Option<(f64, f64)> {
    let bins = histogram.len() as u64;
    let support = u64::from(posterior_draws) + 1;
    let count = histogram.iter().map(|&value| u64::from(value)).sum::<u64>();
    if bins < 2 || bins > support || count == 0 {
        return None;
    }
    let bin_sizes = (0..bins)
        .map(|bin| {
            let lower = (bin * support).div_ceil(bins);
            let upper = ((bin + 1) * support).div_ceil(bins);
            upper - lower
        })
        .collect::<Vec<_>>();
    let expected = bin_sizes
        .iter()
        .map(|size| count as f64 * *size as f64 / support as f64)
        .collect::<Vec<_>>();
    let statistic = histogram
        .iter()
        .zip(&expected)
        .map(|(&observed, &expected)| (f64::from(observed) - expected).powi(2) / expected)
        .sum::<f64>();
    let p_value = if expected.iter().all(|value| *value >= 5.0) {
        let distribution = statrs::distribution::ChiSquared::new((bins - 1) as f64).ok()?;
        statrs::distribution::ContinuousCDF::sf(&distribution, statistic)
    } else if count <= EXACT_MULTINOMIAL_MAXIMUM_COUNT && bins <= EXACT_MULTINOMIAL_MAXIMUM_BINS {
        exact_multinomial_tail(histogram, &bin_sizes, support)?
    } else {
        deterministic_multinomial_tail(histogram, &bin_sizes, support)?
    };
    Some((statistic, p_value))
}

fn exact_multinomial_tail(histogram: &[u32], bin_sizes: &[u64], support: u64) -> Option<f64> {
    let count = histogram.iter().map(|&value| u64::from(value)).sum::<u64>();
    let scale = score_scale(bin_sizes)?;
    let observed_score = histogram_score(histogram, bin_sizes, scale)?;
    let mut states = BTreeMap::from([((0_u64, 0_u128), 1.0_f64)]);
    for &size in bin_sizes {
        let probability = size as f64 / support as f64;
        let multiplier = u128::from(scale / size);
        let mut terms = vec![1.0_f64; count as usize + 1];
        for value in 1..=count as usize {
            terms[value] = terms[value - 1] * probability / value as f64;
        }
        let mut next = BTreeMap::new();
        for ((used, score), weight) in states {
            for added in 0..=count - used {
                let added_score = u128::from(added) * u128::from(added) * multiplier;
                *next.entry((used + added, score + added_score)).or_default() +=
                    weight * terms[added as usize];
            }
        }
        states = next;
    }
    let coefficient = states
        .into_iter()
        .filter(|((used, score), _)| *used == count && *score >= observed_score)
        .map(|(_, weight)| weight)
        .sum::<f64>();
    let factorial = (2..=count).fold(1.0, |value, factor| value * factor as f64);
    let probability = coefficient * factorial;
    probability.is_finite().then(|| {
        if probability >= 1.0 - 1e-12 {
            1.0
        } else {
            probability.clamp(0.0, 1.0)
        }
    })
}

fn deterministic_multinomial_tail(
    histogram: &[u32],
    bin_sizes: &[u64],
    support: u64,
) -> Option<f64> {
    let count = histogram.iter().map(|&value| u64::from(value)).sum::<u64>();
    let bins = histogram.len() as u64;
    let scale = score_scale(bin_sizes)?;
    let observed_score = histogram_score(histogram, bin_sizes, scale)?;
    let mut state = MONTE_CARLO_SEED;
    let rejection_threshold = support.wrapping_neg() % support;
    let mut extreme = 0_u64;
    let mut simulated = BTreeMap::<usize, u32>::new();
    for _ in 0..MONTE_CARLO_REPLICATES {
        simulated.clear();
        for _ in 0..count {
            let rank = loop {
                let candidate = splitmix64(&mut state);
                if candidate >= rejection_threshold {
                    break candidate % support;
                }
            };
            let bin = (rank * bins / support).min(bins - 1) as usize;
            *simulated.entry(bin).or_default() += 1;
        }
        let simulated_score =
            simulated
                .iter()
                .try_fold(0_u128, |score, (&bin, &observed)| -> Option<u128> {
                    let observed = u128::from(observed);
                    score.checked_add(observed * observed * u128::from(scale / bin_sizes[bin]))
                })?;
        if simulated_score >= observed_score {
            extreme += 1;
        }
    }
    Some((extreme + 1) as f64 / (MONTE_CARLO_REPLICATES + 1) as f64)
}

fn score_scale(bin_sizes: &[u64]) -> Option<u64> {
    bin_sizes.iter().try_fold(1_u64, |scale, &size| {
        scale.checked_mul(size / gcd(scale, size))
    })
}

fn histogram_score(histogram: &[u32], bin_sizes: &[u64], scale: u64) -> Option<u128> {
    histogram
        .iter()
        .zip(bin_sizes)
        .try_fold(0_u128, |score, (&observed, &size)| {
            let observed = u128::from(observed);
            score.checked_add(observed * observed * u128::from(scale / size))
        })
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// Simulation-based calibration of the conjugate normal-mean model.
///
/// Each replicate draws `mu ~ Normal(prior_mean, prior_sd)` and then `y_i ~ Normal(mu,
/// known_sigma)`, with both distribution parameters interpreted as SDs. The worker draws
/// independently from the exact conjugate normal posterior, so rank calibration has no MCMC
/// autocorrelation to correct. For `n` observations its posterior variance is `v = 1 / (1 /
/// prior_sd^2 + n / known_sigma^2)` and its mean is `v * (prior_mean / prior_sd^2 + sum(y) /
/// known_sigma^2)`. Replicate and posterior-draw controls do not alter the model.
///
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
    pub backend_capability: BackendCapability,
    pub maturity: ModelMaturity,
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
            || !is_worker_timeout(timeout_seconds)
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
                version: NUMPY_SCIPY_BACKEND,
                python_version: PYTHON_VERSION,
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
                backend_capability: BackendCapability::SimulationBasedCalibration,
                maturity: ModelMaturity::ExperimentalCalibration,
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

/// Rank-histogram, coverage and mean-rank summaries for one parameter across SBC replicates.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SbcParameterDiagnostics {
    pub rank_histogram: Vec<u32>,
    pub rank_uniformity_p_value: f64,
    pub coverage_90: f64,
    pub mean_normalized_rank: f64,
}

impl SbcParameterDiagnostics {
    /// Whether these worker-reported summaries equal the ones recomputed from the replicate
    /// `(rank, covered)` rows: the rank histogram over `rank_bins` equal bins of
    /// `0..=posterior_draws`, the finite-sample-safe uniformity p-value against exact discrete
    /// bin probabilities, interval coverage, and normalized mean rank.
    pub(crate) fn matches_replicates(
        &self,
        rows: impl IntoIterator<Item = (u32, bool)>,
        posterior_draws: u32,
        rank_bins: u32,
    ) -> bool {
        const TOLERANCE: f64 = 1e-9;
        let bins = rank_bins as usize;
        if bins < 2 || posterior_draws == 0 {
            return false;
        }
        let mut histogram = vec![0_u32; bins];
        let (mut count, mut covered, mut rank_sum) = (0_u32, 0_u32, 0_u64);
        for (rank, is_covered) in rows {
            let bin = (rank as usize * bins / (posterior_draws as usize + 1)).min(bins - 1);
            histogram[bin] += 1;
            count += 1;
            covered += u32::from(is_covered);
            rank_sum += u64::from(rank);
        }
        if self.rank_histogram != histogram {
            return false;
        }
        if count == 0 {
            return self.rank_uniformity_p_value == 0.0
                && self.coverage_90 == 0.0
                && self.mean_normalized_rank == 0.0;
        }
        let Some((_, p_value)) = rank_uniformity(&histogram, posterior_draws) else {
            return false;
        };
        let coverage = f64::from(covered) / f64::from(count);
        let mean_normalized_rank = rank_sum as f64 / f64::from(count) / f64::from(posterior_draws);
        relative_eq(self.rank_uniformity_p_value, p_value, TOLERANCE)
            && relative_eq(self.coverage_90, coverage, TOLERANCE)
            && relative_eq(self.mean_normalized_rank, mean_normalized_rank, TOLERANCE)
    }
}

/// SBC controls a caller chooses for a NumPyro calibration run.
#[derive(Clone, Copy, Debug)]
pub struct SbcControls {
    pub replicates: u32,
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

/// Calibration policy recorded in a NumPyro SBC request and its result.
#[derive(Clone, Debug, Serialize)]
pub struct SbcCalibrationPolicy {
    pub replicates: u32,
    pub rank_bins: u32,
    pub interval_probability: f64,
    pub maximum_tree_depth: u32,
    pub minimum_rank_uniformity_p_value: f64,
    pub minimum_coverage: f64,
    pub maximum_coverage: f64,
}

impl SbcCalibrationPolicy {
    /// The policy for `controls`, with 10 rank bins, 90% intervals and a NUTS tree depth of
    /// 12, or `None` when a control is out of range.
    pub fn new(controls: SbcControls) -> Option<Self> {
        let SbcControls {
            replicates,
            minimum_rank_uniformity_p_value,
            minimum_coverage,
            maximum_coverage,
        } = controls;
        let valid = (20..=100).contains(&replicates)
            && minimum_rank_uniformity_p_value.is_finite()
            && (0.0..0.1).contains(&minimum_rank_uniformity_p_value)
            && (0.0..=1.0).contains(&minimum_coverage)
            && (0.0..=1.0).contains(&maximum_coverage)
            && minimum_coverage <= maximum_coverage;
        valid.then_some(Self {
            replicates,
            rank_bins: 10,
            interval_probability: 0.9,
            maximum_tree_depth: 12,
            minimum_rank_uniformity_p_value,
            minimum_coverage,
            maximum_coverage,
        })
    }
}

/// Calibration fields used by the shared validator, independent of the model's JSON layout.
pub(crate) trait SbcCalibration {
    fn replicates(&self) -> u32;
    fn rank_bins(&self) -> u32;
    fn passes(&self, diagnostic: &SbcParameterDiagnostics) -> bool;
}

impl SbcCalibration for SbcCalibrationPolicy {
    fn replicates(&self) -> u32 {
        self.replicates
    }
    fn rank_bins(&self) -> u32 {
        self.rank_bins
    }
    fn passes(&self, diagnostic: &SbcParameterDiagnostics) -> bool {
        diagnostic.rank_uniformity_p_value >= self.minimum_rank_uniformity_p_value
            && (self.minimum_coverage..=self.maximum_coverage).contains(&diagnostic.coverage_90)
    }
}

/// NUTS diagnostics that every SBC replicate reports for its posterior fit.
pub(crate) struct SbcSamplerRow {
    pub(crate) r_hat: f64,
    pub(crate) ess_bulk: f64,
    pub(crate) ess_tail: f64,
    // The original Gaussian hierarchy format omits E-BFMI; its worker enforces that
    // gate before emitting ranks. Other formats report it for independent validation.
    pub(crate) minimum_ebfmi: Option<f64>,
    pub(crate) divergences: u64,
    pub(crate) max_tree_depth_hits: u64,
}

/// One fitted SBC replicate as [`validate_sbc_replicates`] sees it.
pub(crate) trait SbcReplicateRow {
    fn replicate(&self) -> u32;
    /// `(rank, covered)` for each calibrated quantity, in the order of the result's diagnostics.
    fn ranks(&self) -> Vec<(u32, bool)>;
    /// Whether every simulated true value is finite and inside its parameter's support.
    fn truths_valid(&self) -> bool;
    fn sampler(&self) -> SbcSamplerRow;
}

pub(crate) trait SbcFailureRow {
    fn replicate(&self) -> u32;
    fn reason(&self) -> &str;
}

impl SbcFailureRow for HierarchicalSbcFailure {
    fn replicate(&self) -> u32 {
        self.replicate
    }
    fn reason(&self) -> &str {
        &self.reason
    }
}

/// Which part of an SBC worker result broke the contract.
#[derive(Debug)]
pub(crate) enum SbcViolation {
    Replicate,
    Failure,
    Diagnostics,
    Disposition,
}

impl SbcViolation {
    pub(crate) fn error(self, model: &str) -> BayesError {
        let what = match self {
            Self::Replicate => "replicate is invalid",
            Self::Failure => "failure disposition is invalid",
            Self::Diagnostics => "diagnostics are invalid",
            Self::Disposition => "disposition or fit state is invalid",
        };
        BayesError::WorkerContract(format!("{model} SBC {what}"))
    }
}

/// Check an SBC worker's replicates, failures and diagnostics against the request.
///
/// Every replicate index must be disposed exactly once, every fit must meet the request's
/// sampler policy, the per-quantity diagnostics must equal the ones recomputed from the
/// replicate ranks, and the reported fit state must match whether every quantity passes.
pub(crate) fn validate_sbc_replicates<R: SbcReplicateRow>(
    replicates: &[R],
    failures: &[impl SbcFailureRow],
    diagnostics: &[&SbcParameterDiagnostics],
    calibration: &impl SbcCalibration,
    posterior_draws: u32,
    policy: &DiagnosticPolicy,
    fit_state: FitState,
) -> Result<(), SbcViolation> {
    let mut dispositions = BTreeSet::new();
    for row in replicates {
        let ranks = row.ranks();
        let sampler = row.sampler();
        if row.replicate() >= calibration.replicates()
            || !dispositions.insert(row.replicate())
            || ranks.len() != diagnostics.len()
            || ranks.iter().any(|(rank, _)| *rank > posterior_draws)
            || !row.truths_valid()
            || ![sampler.r_hat, sampler.ess_bulk, sampler.ess_tail]
                .iter()
                .all(|value| value.is_finite())
            || sampler.r_hat <= 0.0
            || sampler.ess_bulk <= 0.0
            || sampler.ess_tail <= 0.0
            || sampler.r_hat > policy.maximum_r_hat
            || sampler.ess_bulk < policy.minimum_bulk_ess
            || sampler.ess_tail < policy.minimum_tail_ess
            || sampler.minimum_ebfmi.is_some_and(|value| {
                !value.is_finite() || value < 0.0 || value < policy.minimum_ebfmi
            })
            || sampler.divergences > policy.maximum_divergences
            || sampler.max_tree_depth_hits > policy.maximum_tree_depth_hits
        {
            return Err(SbcViolation::Replicate);
        }
    }
    for failure in failures {
        if failure.replicate() >= calibration.replicates()
            || !dispositions.insert(failure.replicate())
            || failure.reason().is_empty()
            || failure.reason().len() > 512
        {
            return Err(SbcViolation::Failure);
        }
    }
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if !diagnostic.matches_replicates(
            replicates.iter().map(|row| row.ranks()[index]),
            posterior_draws,
            calibration.rank_bins(),
        ) {
            return Err(SbcViolation::Diagnostics);
        }
    }
    let passes = failures.is_empty()
        && diagnostics
            .iter()
            .all(|diagnostic| calibration.passes(diagnostic));
    if dispositions.len() != calibration.replicates() as usize
        || (fit_state == FitState::Complete) != passes
    {
        return Err(SbcViolation::Disposition);
    }
    Ok(())
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
            || !self.backend.matches(&request.backend)
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
        let mut rank_counts = vec![0_u32; request.sbc.posterior_draws as usize + 1];
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
                || !relative_eq(
                    replicate.interval_lower,
                    replicate.posterior_mean - Z_975 * replicate.posterior_sd,
                    RELATIVE_TOLERANCE,
                )
                || !relative_eq(
                    replicate.interval_upper,
                    replicate.posterior_mean + Z_975 * replicate.posterior_sd,
                    RELATIVE_TOLERANCE,
                )
                || replicate.covered
                    != (replicate.true_value >= replicate.interval_lower
                        && replicate.true_value <= replicate.interval_upper)
                || !relative_eq(
                    replicate.z_score,
                    (replicate.true_value - replicate.posterior_mean) / replicate.posterior_sd,
                    RELATIVE_TOLERANCE,
                )
                || !relative_eq(
                    replicate.shrinkage,
                    1.0 - replicate.posterior_sd.powi(2) / request.model.prior_sd.powi(2),
                    RELATIVE_TOLERANCE,
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
            rank_counts[replicate.rank as usize] += 1;
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
        let (rank_uniformity_chi_square, rank_uniformity_p_value) =
            rank_uniformity(&histogram, request.sbc.posterior_draws).ok_or_else(|| {
                BayesError::WorkerContract("normal-mean SBC rank bins are invalid".into())
            })?;
        let mut cumulative = 0_u32;
        let maximum_ecdf_deviation = rank_counts
            .iter()
            .enumerate()
            .map(|(rank, count)| {
                cumulative += count;
                let empirical = f64::from(cumulative) / success_count as f64;
                let expected = (rank + 1) as f64 / rank_counts.len() as f64;
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
            || !relative_eq(
                self.diagnostics.rank_uniformity_chi_square,
                rank_uniformity_chi_square,
                RELATIVE_TOLERANCE,
            )
            || !relative_eq(
                self.diagnostics.rank_uniformity_p_value,
                rank_uniformity_p_value,
                RELATIVE_TOLERANCE,
            )
            || !relative_eq(
                self.diagnostics.maximum_ecdf_deviation,
                maximum_ecdf_deviation,
                RELATIVE_TOLERANCE,
            )
            || !relative_eq(
                self.diagnostics.ecdf_envelope,
                ecdf_envelope,
                RELATIVE_TOLERANCE,
            )
            || !relative_eq(self.diagnostics.coverage_95, coverage, RELATIVE_TOLERANCE)
            || !relative_eq(
                self.diagnostics.coverage_standardized_error,
                coverage_standardized_error,
                RELATIVE_TOLERANCE,
            )
            || !relative_eq(self.diagnostics.z_score_mean, z_mean, RELATIVE_TOLERANCE)
            || !relative_eq(self.diagnostics.z_score_sd, z_sd, RELATIVE_TOLERANCE)
            || !relative_eq(
                self.diagnostics.mean_shrinkage,
                shrinkage,
                RELATIVE_TOLERANCE,
            )
            || !relative_eq(
                self.diagnostics.failure_rate,
                failure_rate,
                RELATIVE_TOLERANCE,
            )
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
            claim_status: ClaimStatus::ExperimentalCalibrationProcedure,
            request_sha256: self.request_sha256,
        }
    }
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
    pub claim_status: ClaimStatus,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::{rank_uniformity, SbcParameterDiagnostics, MONTE_CARLO_REPLICATES};

    // Ranks 0, 5, 9 of 9 draws in 2 bins give histogram [1, 2]. Every possible three-draw
    // histogram is at least this far from the uniform null, so its exact tail probability is one.
    const ROWS: [(u32, bool); 3] = [(0, true), (5, false), (9, true)];

    fn reported(p_value: f64) -> SbcParameterDiagnostics {
        SbcParameterDiagnostics {
            rank_histogram: vec![1, 2],
            rank_uniformity_p_value: p_value,
            coverage_90: 2.0 / 3.0,
            mean_normalized_rank: 14.0 / 3.0 / 9.0,
        }
    }

    #[test]
    fn accepts_summaries_recomputed_from_the_replicates() {
        assert!(reported(1.0).matches_replicates(ROWS, 9, 2));

        // 21 equally frequent ranks mapped to 20 bins produce a 2:1 first-bin count.
        let mut histogram = vec![100; 20];
        histogram[0] = 200;
        let uniform = SbcParameterDiagnostics {
            rank_histogram: histogram,
            rank_uniformity_p_value: 1.0,
            coverage_90: 1.0,
            mean_normalized_rank: 0.5,
        };
        assert!(uniform.matches_replicates((0..=20).cycle().take(2100).map(|r| (r, true)), 20, 20));
    }

    #[test]
    fn rejects_worker_summaries_that_disagree_with_the_replicates() {
        assert!(!reported(0.6).matches_replicates(ROWS, 9, 2));
        let mut wrong_histogram = reported(1.0);
        wrong_histogram.rank_histogram = vec![2, 1];
        assert!(!wrong_histogram.matches_replicates(ROWS, 9, 2));
        let mut wrong_coverage = reported(1.0);
        wrong_coverage.coverage_90 = 1.0;
        assert!(!wrong_coverage.matches_replicates(ROWS, 9, 2));
    }

    #[test]
    fn sparse_rank_histogram_uses_the_exact_multinomial_tail() {
        // With 200 posterior draws, the 201 ranks occupy bins of sizes 21, 20, ..., 20.
        // Pearson's asymptotic chi-square tail is 0.0099377856 for this histogram, while
        // enumerating the multinomial null gives 0.0101682256 and changes a 0.01 gate.
        let histogram = [8, 1, 0, 2, 1, 0, 2, 2, 2, 2];
        let (_, p_value) = rank_uniformity(&histogram, 200).expect("valid histogram");
        assert!((p_value - 0.010_168_225_602_421_036).abs() < 1e-12);
    }

    #[test]
    fn larger_sparse_rank_histogram_has_a_replayable_monte_carlo_tail() {
        let histogram = [10, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4];
        let first = rank_uniformity(&histogram, 200).expect("valid histogram");
        let replay = rank_uniformity(&histogram, 200).expect("replayed histogram");
        assert_eq!(first, replay);
        assert!((first.1 - 0.305_834_708_264_586_75).abs() < 1e-15);
    }

    #[test]
    fn wide_sparse_rank_histogram_uses_bounded_monte_carlo_tail() {
        let mut histogram = vec![0; 128];
        histogram[0] = 20;
        let (_, p_value) = rank_uniformity(&histogram, 127).expect("valid histogram");
        assert_eq!(p_value, 1.0 / (MONTE_CARLO_REPLICATES + 1) as f64);
    }

    #[test]
    fn no_completed_replicates_require_zero_summaries() {
        let empty = SbcParameterDiagnostics {
            rank_histogram: vec![0, 0],
            rank_uniformity_p_value: 0.0,
            coverage_90: 0.0,
            mean_normalized_rank: 0.0,
        };
        assert!(empty.matches_replicates([], 9, 2));
    }
}

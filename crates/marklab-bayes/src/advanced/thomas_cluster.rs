//! Bayesian Thomas cluster process behind `bayes advanced-cluster` and its simulation-based
//! calibration (`bayes advanced-cluster-sbc`).
//!
//! For each pattern `j` with rectangular window `W_j`, parents form a homogeneous Poisson
//! process of intensity `kappa_j` on `B_j`, the rectangle `W_j` widened by the declared
//! dilation on every side. Given the parents, the observed points form a Poisson process on
//! `W_j` with intensity `mu_j * sum_c N2(u; c, sigma_j^2 I)`. The log parameters are
//! independent `Normal(eta_p, tau_p^2)` across patterns, with `eta_p ~ Normal(m_p, s_p^2)` and
//! `tau_p ~ HalfNormal(h_p)`. The worker samples parents by spatial birth-death-move
//! Metropolis-Hastings and the parameters by Metropolis and Gibbs updates, then reports ArviZ
//! rank R-hat and bulk/tail ESS over all chains. Parents beyond `B_j` are excluded by the model;
//! the result reports the exact share of offspring intensity that they would contribute.
//!
//! Format `marklab.bayesian_thomas_cluster_model` version 1 replaces the earlier heuristic
//! output `marklab.advanced_cluster_and_gibbs_models` version 2, which
//! [`ClusterModelDocument::decode`] still reads.

use std::collections::BTreeSet;

use marklab_core::backend::versions::{PYTHON_VERSION, SCIPY_BACKEND};
use marklab_core::backend::{is_worker_timeout, BackendContract, ClaimStatus, FitState};
use marklab_core::backend::{WorkerBackend, WorkerError};
use serde::{Deserialize, Serialize};

use super::AdvancedClusterResult;
use crate::sbc::rank_uniformity;

/// Request envelope read by the Bayesian Thomas worker.
pub const THOMAS_REQUEST_FORMAT: &str = "marklab.bayesian_thomas_request";
/// Published posterior format of `bayes advanced-cluster`.
pub const THOMAS_RESULT_FORMAT: &str = "marklab.bayesian_thomas_cluster_model";
/// Published calibration format of `bayes advanced-cluster-sbc`.
pub const THOMAS_SBC_RESULT_FORMAT: &str = "marklab.bayesian_thomas_cluster_sbc";
/// Worker script, relative to `workers/python`.
pub const THOMAS_WORKER: &str = "marklab_scipy_bayesian_thomas_worker.py";
const LEGACY_CLUSTER_FORMAT: &str = "marklab.advanced_cluster_and_gibbs_models";
const BACKEND_NAME: &str = "numpy_scipy_arviz";
const MAXIMUM_PATTERNS: usize = 64;
const MAXIMUM_PARENTS: u32 = 100_000;
const MAXIMUM_POINTS: u32 = 1_000_000;
const MAXIMUM_KERNEL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAXIMUM_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;
const P_VALUE_TOLERANCE: f64 = 1e-8;

/// Axis-aligned window in micrometres.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RectangleUm {
    pub xmin_um: f64,
    pub xmax_um: f64,
    pub ymin_um: f64,
    pub ymax_um: f64,
}

impl RectangleUm {
    fn is_valid(&self) -> bool {
        [self.xmin_um, self.xmax_um, self.ymin_um, self.ymax_um]
            .into_iter()
            .all(f64::is_finite)
            && self.xmin_um < self.xmax_um
            && self.ymin_um < self.ymax_um
    }

    fn contains(&self, [x, y]: [f64; 2]) -> bool {
        self.xmin_um <= x && x <= self.xmax_um && self.ymin_um <= y && y <= self.ymax_um
    }

    /// Area in square micrometres.
    pub fn area(&self) -> f64 {
        (self.xmax_um - self.xmin_um) * (self.ymax_um - self.ymin_um)
    }

    /// Area of the window widened by `dilation` on every side, evaluated as the worker does.
    fn dilated_area(&self, dilation: f64) -> f64 {
        ((self.xmax_um + dilation) - (self.xmin_um - dilation))
            * ((self.ymax_um + dilation) - (self.ymin_um - dilation))
    }
}

/// Prior of one log parameter: `eta ~ Normal(population_mean_mean, population_mean_sd^2)` and
/// `tau ~ HalfNormal(between_pattern_sd_scale)`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogNormalHierarchyPrior {
    pub population_mean_mean: f64,
    pub population_mean_sd: f64,
    pub between_pattern_sd_scale: f64,
}

impl LogNormalHierarchyPrior {
    fn is_valid(&self) -> bool {
        self.population_mean_mean.is_finite()
            && self.population_mean_mean.abs() <= 50.0
            && (1e-3..=10.0).contains(&self.population_mean_sd)
            && (1e-3..=10.0).contains(&self.between_pattern_sd_scale)
    }
}

/// Priors of `log kappa` (parents per square micrometre), `log mu` (mean offspring per parent)
/// and `log sigma` (offspring displacement SD in micrometres).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasPriors {
    pub log_kappa: LogNormalHierarchyPrior,
    pub log_mu: LogNormalHierarchyPrior,
    pub log_sigma: LogNormalHierarchyPrior,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasSampler {
    pub chains: u32,
    pub warmup: u32,
    pub draws: u32,
    pub birth_death_proposals_per_sweep: u32,
    pub seed: u64,
}

impl ThomasSampler {
    fn is_valid(&self) -> bool {
        (2..=16).contains(&self.chains)
            && (50..=20_000).contains(&self.warmup)
            && (50..=20_000).contains(&self.draws)
            && (1..=10_000).contains(&self.birth_death_proposals_per_sweep)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasDiagnosticPolicy {
    pub maximum_r_hat: f64,
    pub minimum_bulk_ess: f64,
    pub minimum_tail_ess: f64,
    pub maximum_edge_truncation_fraction: f64,
}

impl ThomasDiagnosticPolicy {
    fn is_valid(&self) -> bool {
        (1.0..=2.0).contains(&self.maximum_r_hat)
            && (1.0..=1e9).contains(&self.minimum_bulk_ess)
            && (1.0..=1e9).contains(&self.minimum_tail_ess)
            && (0.0..=1.0).contains(&self.maximum_edge_truncation_fraction)
    }
}

/// Caller-declared limits checked before the worker allocates kernel storage or starts sampling.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasResources {
    pub maximum_parents_per_pattern: u32,
    pub maximum_points_per_pattern: u32,
    pub maximum_kernel_bytes: u64,
    pub maximum_work_units: u64,
    pub maximum_output_bytes: u64,
}

impl ThomasResources {
    fn is_valid(&self) -> bool {
        (1..=MAXIMUM_PARENTS).contains(&self.maximum_parents_per_pattern)
            && (2..=MAXIMUM_POINTS).contains(&self.maximum_points_per_pattern)
            && (1..=MAXIMUM_KERNEL_BYTES).contains(&self.maximum_kernel_bytes)
            && self.maximum_work_units >= 1
            && (1..=MAXIMUM_OUTPUT_BYTES).contains(&self.maximum_output_bytes)
    }

    /// Upper bound on kernel evaluations: per sweep and pattern, `n` per birth-death proposal
    /// and `8 n` per parent slot for displacements, scale updates and the exact refresh.
    fn work_units(&self, sampler: &ThomasSampler, point_counts: &[usize]) -> Option<u128> {
        let sweeps =
            u128::from(sampler.chains) * (u128::from(sampler.warmup) + u128::from(sampler.draws));
        let per_point = u128::from(sampler.birth_death_proposals_per_sweep)
            + 8 * u128::from(self.maximum_parents_per_pattern);
        let points = point_counts
            .iter()
            .map(|&count| count as u128)
            .sum::<u128>();
        sweeps.checked_mul(per_point)?.checked_mul(points)
    }

    /// Current and proposed point-by-parent kernel matrices of every pattern, in bytes.
    fn kernel_bytes(&self, point_counts: &[usize]) -> u128 {
        let points = point_counts
            .iter()
            .map(|&count| count as u128)
            .sum::<u128>();
        16 * u128::from(self.maximum_parents_per_pattern) * points
    }

    fn check(
        &self,
        sampler: &ThomasSampler,
        point_counts: &[usize],
        repeats: u32,
    ) -> Result<(u64, u64), WorkerError> {
        let work = self
            .work_units(sampler, point_counts)
            .and_then(|work| work.checked_mul(u128::from(repeats)))
            .filter(|&work| work <= u128::from(self.maximum_work_units))
            .ok_or_else(|| {
                WorkerError::InvalidSpec("declared sampler work exceeds maximum_work_units".into())
            })?;
        let memory = self.kernel_bytes(point_counts);
        if memory > u128::from(self.maximum_kernel_bytes) {
            return Err(WorkerError::InvalidSpec(
                "parent kernel storage exceeds maximum_kernel_bytes".into(),
            ));
        }
        Ok((work as u64, memory as u64))
    }
}

/// One patient's observed pattern.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasPattern {
    pub pattern_id: String,
    pub patient_id: String,
    pub window: RectangleUm,
    pub points: Vec<[f64; 2]>,
}

/// Optional finite binary Gibbs exchange fit, independent of the cluster model.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FiniteGibbsInput {
    pub sites: u32,
    pub edges: Vec<[u32; 2]>,
    pub observed_types: Vec<u8>,
    pub interaction_prior_sd: f64,
    pub proposal_sd: f64,
    pub draws: u32,
    pub burnin: u32,
    pub seed: u64,
}

impl FiniteGibbsInput {
    fn is_valid(&self) -> bool {
        let types = self.observed_types.iter().copied().collect::<BTreeSet<_>>();
        (2..=16).contains(&self.sites)
            && self.observed_types.len() == self.sites as usize
            && types == BTreeSet::from([0, 1])
            && self
                .edges
                .iter()
                .all(|&[first, second]| first < second && second < self.sites)
            && (1e-6..=1e6).contains(&self.interaction_prior_sd)
            && (1e-6..=1e6).contains(&self.proposal_sd)
            && (100..=100_000).contains(&self.draws)
            && self.burnin <= 100_000
    }
}

/// `bayes advanced-cluster` input document.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasClusterInput {
    pub patterns: Vec<ThomasPattern>,
    pub dilation_um: f64,
    pub priors: ThomasPriors,
    pub sampler: ThomasSampler,
    pub diagnostic_policy: ThomasDiagnosticPolicy,
    pub resources: ThomasResources,
    pub posterior_predictive_draws: u32,
    #[serde(default)]
    pub finite_gibbs: Option<FiniteGibbsInput>,
    pub timeout_seconds: u64,
}

/// `bayes advanced-cluster-sbc` input document.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasClusterSbcInput {
    pub window: RectangleUm,
    pub dilation_um: f64,
    pub priors: ThomasPriors,
    pub sampler: ThomasSampler,
    pub diagnostic_policy: ThomasDiagnosticPolicy,
    pub resources: ThomasResources,
    pub patterns_per_replicate: u32,
    pub replicates: u32,
    pub minimum_points: u32,
    pub maximum_points: u32,
    pub maximum_prior_attempts: u32,
    pub rank_draws: u32,
    pub rank_bins: u32,
    pub minimum_rank_p_value: f64,
    pub coverage_90_bounds: [f64; 2],
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThomasModelIr {
    pub family: &'static str,
    pub parent_process: &'static str,
    pub offspring_kernel: &'static str,
    pub pooling: &'static str,
    pub parent_sampler: &'static str,
    pub dilation_um: f64,
    pub priors: ThomasPriors,
}

impl ThomasModelIr {
    fn new(dilation_um: f64, priors: ThomasPriors) -> Self {
        Self {
            family: "thomas_cluster_process",
            parent_process: "homogeneous_poisson_on_rectangular_dilation",
            offspring_kernel: "isotropic_gaussian",
            pooling: "lognormal_hierarchy_over_patterns",
            parent_sampler: "spatial_birth_death_move_metropolis_hastings",
            dilation_um,
            priors,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ThomasRequestResources {
    pub maximum_parents_per_pattern: u32,
    pub maximum_points_per_pattern: u32,
    pub maximum_kernel_bytes: u64,
    pub maximum_work_units: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThomasFitPayload {
    pub patterns: Vec<ThomasPattern>,
    pub posterior_predictive_draws: u32,
    pub finite_gibbs: Option<FiniteGibbsInput>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThomasSbcSpec {
    pub window: RectangleUm,
    pub patterns_per_replicate: u32,
    pub replicates: u32,
    pub minimum_points: u32,
    pub maximum_points: u32,
    pub maximum_prior_attempts: u32,
    pub rank_draws: u32,
    pub rank_bins: u32,
    pub minimum_rank_p_value: f64,
    pub coverage_90_bounds: [f64; 2],
}

#[derive(Clone, Debug, Serialize)]
pub struct ThomasSbcPayload {
    pub sbc: ThomasSbcSpec,
}

/// Worker request; `P` is the mode-specific payload.
#[derive(Clone, Debug, Serialize)]
pub struct ThomasClusterRequest<P> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub mode: &'static str,
    pub model: ThomasModelIr,
    pub sampler: ThomasSampler,
    pub diagnostic_policy: ThomasDiagnosticPolicy,
    pub resources: ThomasRequestResources,
    #[serde(flatten)]
    pub payload: P,
    /// Work units and kernel bytes checked against the declared resources.
    #[serde(skip)]
    pub checked_budget: (u64, u64),
}

fn backend(environment_lock_sha256: String, worker_sha256: String) -> BackendContract {
    BackendContract {
        name: BACKEND_NAME,
        version: SCIPY_BACKEND,
        python_version: PYTHON_VERSION,
        environment_lock_sha256,
        worker_sha256,
    }
}

fn common_is_valid(
    dilation_um: f64,
    priors: &ThomasPriors,
    sampler: &ThomasSampler,
    policy: &ThomasDiagnosticPolicy,
    resources: &ThomasResources,
    timeout_seconds: u64,
) -> bool {
    dilation_um.is_finite()
        && (1e-9..=1e9).contains(&dilation_um)
        && [&priors.log_kappa, &priors.log_mu, &priors.log_sigma]
            .into_iter()
            .all(LogNormalHierarchyPrior::is_valid)
        && sampler.is_valid()
        && policy.is_valid()
        && resources.is_valid()
        && is_worker_timeout(timeout_seconds)
}

fn request_resources(resources: &ThomasResources, timeout_seconds: u64) -> ThomasRequestResources {
    ThomasRequestResources {
        maximum_parents_per_pattern: resources.maximum_parents_per_pattern,
        maximum_points_per_pattern: resources.maximum_points_per_pattern,
        maximum_kernel_bytes: resources.maximum_kernel_bytes,
        maximum_work_units: resources.maximum_work_units,
        maximum_output_bytes: resources.maximum_output_bytes,
        timeout_seconds,
    }
}

impl ThomasClusterRequest<ThomasFitPayload> {
    /// Validate a fit input and bound its work and kernel memory before any worker runs.
    pub fn fit(
        input: ThomasClusterInput,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, WorkerError> {
        let invalid = |message: &str| WorkerError::InvalidSpec(message.into());
        if !common_is_valid(
            input.dilation_um,
            &input.priors,
            &input.sampler,
            &input.diagnostic_policy,
            &input.resources,
            input.timeout_seconds,
        ) {
            return Err(invalid(
                "Thomas cluster priors, dilation, sampler, policy, resources, or timeout are invalid",
            ));
        }
        if !(1..=MAXIMUM_PATTERNS).contains(&input.patterns.len()) {
            return Err(invalid("Thomas cluster input requires 1-64 patterns"));
        }
        let pattern_ids = input
            .patterns
            .iter()
            .map(|pattern| pattern.pattern_id.as_str())
            .collect::<BTreeSet<_>>();
        let patient_ids = input
            .patterns
            .iter()
            .map(|pattern| pattern.patient_id.as_str())
            .collect::<BTreeSet<_>>();
        if pattern_ids.len() != input.patterns.len() || patient_ids.len() != input.patterns.len() {
            return Err(invalid(
                "pattern and patient identifiers must be unique (one pattern per patient)",
            ));
        }
        let maximum_points = input.resources.maximum_points_per_pattern as usize;
        for pattern in &input.patterns {
            if !pattern.window.is_valid()
                || !(2..=maximum_points).contains(&pattern.points.len())
                || !pattern.points.iter().all(|&point| {
                    point.iter().all(|value| value.is_finite()) && pattern.window.contains(point)
                })
            {
                return Err(invalid(
                    "each pattern needs a positive window and 2..=maximum_points_per_pattern finite points inside it",
                ));
            }
        }
        let retained = u64::from(input.sampler.chains) * u64::from(input.sampler.draws);
        if !(10..=2_000).contains(&input.posterior_predictive_draws)
            || u64::from(input.posterior_predictive_draws) > retained
        {
            return Err(invalid(
                "posterior_predictive_draws must be 10-2000 and at most the retained draws",
            ));
        }
        if input
            .finite_gibbs
            .as_ref()
            .is_some_and(|gibbs| !gibbs.is_valid())
        {
            return Err(invalid("finite_gibbs contract is invalid"));
        }
        let counts = input
            .patterns
            .iter()
            .map(|pattern| pattern.points.len())
            .collect::<Vec<_>>();
        let checked_budget = input.resources.check(&input.sampler, &counts, 1)?;
        Ok(Self {
            format: THOMAS_REQUEST_FORMAT,
            version: 1,
            backend: backend(environment_lock_sha256, worker_sha256),
            mode: "fit",
            model: ThomasModelIr::new(input.dilation_um, input.priors),
            sampler: input.sampler,
            diagnostic_policy: input.diagnostic_policy,
            resources: request_resources(&input.resources, input.timeout_seconds),
            payload: ThomasFitPayload {
                patterns: input.patterns,
                posterior_predictive_draws: input.posterior_predictive_draws,
                finite_gibbs: input.finite_gibbs,
            },
            checked_budget,
        })
    }
}

impl ThomasClusterRequest<ThomasSbcPayload> {
    /// Validate an SBC input and bound the work of every replicate before any worker runs.
    pub fn sbc(
        input: ThomasClusterSbcInput,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, WorkerError> {
        let [coverage_low, coverage_high] = input.coverage_90_bounds;
        if !common_is_valid(
            input.dilation_um,
            &input.priors,
            &input.sampler,
            &input.diagnostic_policy,
            &input.resources,
            input.timeout_seconds,
        ) || !input.window.is_valid()
            || !(1..=16).contains(&input.patterns_per_replicate)
            || !(1..=2_000).contains(&input.replicates)
            || input.minimum_points < 2
            || input.minimum_points > input.maximum_points
            || input.maximum_points > input.resources.maximum_points_per_pattern
            || !(1..=1_000_000).contains(&input.maximum_prior_attempts)
            || !(9..=999).contains(&input.rank_draws)
            || !(2..=50).contains(&input.rank_bins)
            || !(input.rank_draws + 1).is_multiple_of(input.rank_bins)
            || !(0.0..=1.0).contains(&input.minimum_rank_p_value)
            || !(0.0..=1.0).contains(&coverage_low)
            || !(coverage_low..=1.0).contains(&coverage_high)
        {
            return Err(WorkerError::InvalidSpec(
                "Thomas cluster SBC window, replicate, rank, coverage, or model controls are invalid"
                    .into(),
            ));
        }
        let counts = vec![input.maximum_points as usize; input.patterns_per_replicate as usize];
        let checked_budget = input
            .resources
            .check(&input.sampler, &counts, input.replicates)?;
        Ok(Self {
            format: THOMAS_REQUEST_FORMAT,
            version: 1,
            backend: backend(environment_lock_sha256, worker_sha256),
            mode: "sbc",
            model: ThomasModelIr::new(input.dilation_um, input.priors),
            sampler: input.sampler,
            diagnostic_policy: input.diagnostic_policy,
            resources: request_resources(&input.resources, input.timeout_seconds),
            payload: ThomasSbcPayload {
                sbc: ThomasSbcSpec {
                    window: input.window,
                    patterns_per_replicate: input.patterns_per_replicate,
                    replicates: input.replicates,
                    minimum_points: input.minimum_points,
                    maximum_points: input.maximum_points,
                    maximum_prior_attempts: input.maximum_prior_attempts,
                    rank_draws: input.rank_draws,
                    rank_bins: input.rank_bins,
                    minimum_rank_p_value: input.minimum_rank_p_value,
                    coverage_90_bounds: input.coverage_90_bounds,
                },
            },
            checked_budget,
        })
    }
}

/// Posterior summary with ArviZ rank R-hat, bulk/tail ESS and mean MCSE over all chains.
/// A diagnostic is `None` when ArviZ cannot compute it, for example for a chain that never moved.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PosteriorQuantity {
    pub mean: f64,
    pub sd: f64,
    pub quantile_025: f64,
    pub median: f64,
    pub quantile_975: f64,
    pub r_hat: Option<f64>,
    pub ess_bulk: Option<f64>,
    pub ess_tail: Option<f64>,
    pub mcse_mean: Option<f64>,
}

impl PosteriorQuantity {
    fn moments_are_valid(&self, positive: bool) -> bool {
        [
            self.mean,
            self.sd,
            self.quantile_025,
            self.median,
            self.quantile_975,
        ]
        .into_iter()
        .all(f64::is_finite)
            && self.sd >= 0.0
            && self.quantile_025 <= self.median
            && self.median <= self.quantile_975
            && (!positive || self.quantile_025 > 0.0)
            && [self.r_hat, self.ess_bulk, self.ess_tail, self.mcse_mean]
                .into_iter()
                .flatten()
                .all(|value| value.is_finite() && value >= 0.0)
    }

    fn diagnostics(&self) -> Option<(f64, f64, f64)> {
        self.mcse_mean?;
        Some((self.r_hat?, self.ess_bulk?, self.ess_tail?))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PosteriorMoments {
    pub mean: f64,
    pub sd: f64,
    pub quantile_025: f64,
    pub median: f64,
    pub quantile_975: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasPatternPosterior {
    pub pattern_id: String,
    pub patient_id: String,
    pub point_count: usize,
    pub window_area_um2: f64,
    pub parent_region_area_um2: f64,
    pub kappa_parent_per_um2: PosteriorQuantity,
    pub mu_offspring: PosteriorQuantity,
    pub sigma_um: PosteriorQuantity,
    pub offspring_intensity_per_um2: PosteriorQuantity,
    pub parent_count_in_region: PosteriorMoments,
    pub maximum_edge_truncation_fraction: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationParameter {
    /// `exp(eta_p)`, the population median of the parameter.
    pub population_median: PosteriorQuantity,
    /// `tau_p`, the between-pattern SD of the log parameter.
    pub between_pattern_sd: PosteriorQuantity,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasPopulation {
    pub log_kappa: PopulationParameter,
    pub log_mu: PopulationParameter,
    pub log_sigma: PopulationParameter,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasAcceptance {
    pub parameter: f64,
    pub shift: f64,
    pub spread: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasDiagnostics {
    pub chains: u32,
    pub draws_per_chain: u32,
    pub warmup_per_chain: u32,
    pub maximum_r_hat: Option<f64>,
    pub minimum_ess_bulk: Option<f64>,
    pub minimum_ess_tail: Option<f64>,
    pub parent_capacity_hits: u64,
    pub maximum_edge_truncation_fraction: f64,
    pub monitored_quantities: usize,
    pub mean_acceptance: ThomasAcceptance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasPredictiveCheck {
    pub pattern_id: String,
    pub replicates: u32,
    pub overflow_replicates: u32,
    pub observed_count: usize,
    pub count_upper_tail_probability: f64,
    pub observed_mean_nearest_neighbour_um: f64,
    pub nearest_neighbour_replicates: u32,
    pub nearest_neighbour_upper_tail_probability: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasModelReport {
    pub family: String,
    pub parent_process: String,
    pub offspring_kernel: String,
    pub pooling: String,
    pub parent_sampler: String,
    pub dilation_um: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FiniteGibbsExchange {
    pub posterior_interaction_mean: f64,
    pub posterior_interaction_interval_95: [f64; 2],
    pub acceptance_rate: f64,
    pub observed_same_type_edge_statistic: f64,
    pub state_space_size: usize,
    pub auxiliary_draw_status: String,
    pub relationship_to_cluster_model: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasResourceReport {
    pub work_units: u64,
    pub kernel_bytes: u64,
    pub maximum_parents_per_pattern: u32,
}

/// Published `bayes advanced-cluster` result.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasClusterResult {
    pub format: String,
    pub version: u32,
    pub fit_state: FitState,
    pub nonconvergence_reasons: Vec<String>,
    pub model: ThomasModelReport,
    pub patterns: Vec<ThomasPatternPosterior>,
    pub population: ThomasPopulation,
    pub diagnostics: ThomasDiagnostics,
    pub posterior_predictive: Vec<ThomasPredictiveCheck>,
    pub resources: ThomasResourceReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finite_binary_gibbs_exchange: Option<FiniteGibbsExchange>,
    pub limitations: Vec<String>,
    pub claim_status: ClaimStatus,
    pub backend: WorkerBackend,
    pub request_sha256: String,
}

fn contract(message: &str) -> WorkerError {
    WorkerError::WorkerContract(message.into())
}

fn same_identity(
    format: &str,
    version: u32,
    backend: &WorkerBackend,
    request_sha256: &str,
    expected: (&str, &BackendContract, &str),
) -> bool {
    format == expected.0
        && version == 1
        && backend.matches(expected.1)
        && request_sha256 == expected.2
}

fn probability(value: f64) -> bool {
    (0.0..=1.0).contains(&value)
}

impl ThomasClusterResult {
    /// Check identity, per-pattern structure and that the reported fit state, reasons and
    /// claim follow from the diagnostics and the request's policy.
    pub fn validate(
        &self,
        request: &ThomasClusterRequest<ThomasFitPayload>,
        request_sha256: &str,
    ) -> Result<(), WorkerError> {
        if !same_identity(
            &self.format,
            self.version,
            &self.backend,
            &self.request_sha256,
            (THOMAS_RESULT_FORMAT, &request.backend, request_sha256),
        ) {
            return Err(contract("Thomas cluster result identity mismatch"));
        }
        let model = &self.model;
        if model.family != request.model.family
            || model.parent_process != request.model.parent_process
            || model.offspring_kernel != request.model.offspring_kernel
            || model.pooling != request.model.pooling
            || model.parent_sampler != request.model.parent_sampler
            || model.dilation_um != request.model.dilation_um
        {
            return Err(contract(
                "Thomas cluster result model differs from the request",
            ));
        }
        let patterns = &request.payload.patterns;
        let dilation = request.model.dilation_um;
        if self.patterns.len() != patterns.len()
            || self.posterior_predictive.len() != patterns.len()
        {
            return Err(contract("Thomas cluster result pattern count differs"));
        }
        let mut worst_truncation = 0.0_f64;
        let mut monitored = Vec::new();
        for ((row, pattern), check) in self
            .patterns
            .iter()
            .zip(patterns)
            .zip(&self.posterior_predictive)
        {
            let quantities = [
                &row.kappa_parent_per_um2,
                &row.mu_offspring,
                &row.sigma_um,
                &row.offspring_intensity_per_um2,
            ];
            if row.pattern_id != pattern.pattern_id
                || row.patient_id != pattern.patient_id
                || row.point_count != pattern.points.len()
                || row.window_area_um2 != pattern.window.area()
                || row.parent_region_area_um2 != pattern.window.dilated_area(dilation)
                || !quantities
                    .iter()
                    .all(|quantity| quantity.moments_are_valid(true))
                || !probability(row.maximum_edge_truncation_fraction)
                || row.parent_count_in_region.quantile_025 < 0.0
                || row.parent_count_in_region.quantile_975.is_nan()
                || row.parent_count_in_region.quantile_975
                    > f64::from(request.resources.maximum_parents_per_pattern)
            {
                return Err(contract("Thomas cluster pattern posterior is invalid"));
            }
            if check.pattern_id != pattern.pattern_id
                || check.replicates != request.payload.posterior_predictive_draws
                || check.overflow_replicates > check.replicates
                || check.nearest_neighbour_replicates > check.replicates
                || check.observed_count != pattern.points.len()
                || !check.observed_mean_nearest_neighbour_um.is_finite()
                || !probability(check.count_upper_tail_probability)
                || !probability(check.nearest_neighbour_upper_tail_probability)
            {
                return Err(contract(
                    "Thomas cluster posterior predictive check is invalid",
                ));
            }
            worst_truncation = worst_truncation.max(row.maximum_edge_truncation_fraction);
            monitored.extend([&row.kappa_parent_per_um2, &row.mu_offspring, &row.sigma_um]);
        }
        for parameter in [
            &self.population.log_kappa,
            &self.population.log_mu,
            &self.population.log_sigma,
        ] {
            if !parameter.population_median.moments_are_valid(true)
                || !parameter.between_pattern_sd.moments_are_valid(false)
            {
                return Err(contract("Thomas cluster population posterior is invalid"));
            }
            monitored.extend([&parameter.population_median, &parameter.between_pattern_sd]);
        }
        self.validate_state(request, &monitored, worst_truncation)?;
        if self.finite_binary_gibbs_exchange.is_some() != request.payload.finite_gibbs.is_some() {
            return Err(contract(
                "finite Gibbs exchange presence differs from the request",
            ));
        }
        if self.resources.work_units != request.checked_budget.0
            || self.resources.kernel_bytes != request.checked_budget.1
            || self.resources.maximum_parents_per_pattern
                != request.resources.maximum_parents_per_pattern
        {
            return Err(contract(
                "Thomas cluster resource report differs from the request",
            ));
        }
        Ok(())
    }

    fn validate_state(
        &self,
        request: &ThomasClusterRequest<ThomasFitPayload>,
        monitored: &[&PosteriorQuantity],
        worst_truncation: f64,
    ) -> Result<(), WorkerError> {
        let policy = &request.diagnostic_policy;
        let diagnostics = &self.diagnostics;
        let summaries = monitored
            .iter()
            .map(|quantity| quantity.diagnostics())
            .collect::<Option<Vec<_>>>();
        let worst = summaries.as_ref().map(|rows| {
            rows.iter().fold(
                (f64::NEG_INFINITY, f64::INFINITY, f64::INFINITY),
                |(r_hat, bulk, tail), row| (r_hat.max(row.0), bulk.min(row.1), tail.min(row.2)),
            )
        });
        let reported = diagnostics
            .maximum_r_hat
            .zip(diagnostics.minimum_ess_bulk)
            .zip(diagnostics.minimum_ess_tail)
            .map(|((r_hat, bulk), tail)| (r_hat, bulk, tail));
        if diagnostics.chains != request.sampler.chains
            || diagnostics.draws_per_chain != request.sampler.draws
            || diagnostics.warmup_per_chain != request.sampler.warmup
            || diagnostics.monitored_quantities != monitored.len()
            || worst != reported
            || diagnostics.maximum_edge_truncation_fraction != worst_truncation
            || ![
                diagnostics.mean_acceptance.parameter,
                diagnostics.mean_acceptance.shift,
                diagnostics.mean_acceptance.spread,
            ]
            .into_iter()
            .all(probability)
        {
            return Err(contract("Thomas cluster diagnostics are inconsistent"));
        }
        let mut reasons = Vec::new();
        match worst {
            None => reasons.push("nonfinite_diagnostic"),
            Some((r_hat, bulk, tail)) => {
                if r_hat > policy.maximum_r_hat {
                    reasons.push("r_hat_above_policy");
                }
                if bulk < policy.minimum_bulk_ess {
                    reasons.push("bulk_ess_below_policy");
                }
                if tail < policy.minimum_tail_ess {
                    reasons.push("tail_ess_below_policy");
                }
            }
        }
        if diagnostics.parent_capacity_hits > 0 {
            reasons.push("parent_capacity_reached");
        }
        if worst_truncation > policy.maximum_edge_truncation_fraction {
            reasons.push("edge_truncation_above_policy");
        }
        let (state, claim) = if reasons.is_empty() {
            (
                FitState::Complete,
                ClaimStatus::ResearchOnlyBayesianThomasClusterPosterior,
            )
        } else {
            (
                FitState::Nonconverged,
                ClaimStatus::DiagnosticOnlyNonconverged,
            )
        };
        if self.nonconvergence_reasons != reasons
            || self.fit_state != state
            || self.claim_status != claim
            || self.limitations.is_empty()
        {
            return Err(contract(
                "Thomas cluster fit state, reasons, or claim do not follow from the diagnostics",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasSbcQuantity {
    pub quantity: String,
    pub rank_histogram: Vec<u32>,
    pub rank_uniformity_p_value: f64,
    pub coverage_90: f64,
    pub mean_normalized_rank: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasSbcReplicate {
    pub replicate: u32,
    pub point_counts: Vec<usize>,
    pub maximum_r_hat: Option<f64>,
    pub minimum_ess_bulk: Option<f64>,
    pub converged: bool,
    pub parent_capacity_hits: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasSbcResources {
    pub work_units: u64,
    pub kernel_bytes: u64,
}

/// Published `bayes advanced-cluster-sbc` result.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasClusterSbcResult {
    pub format: String,
    pub version: u32,
    pub calibration_state: String,
    pub model: ThomasModelReport,
    pub replicates: u32,
    pub nonconverged_replicates: u32,
    pub rejected_prior_draws: u64,
    pub rejection_rule: String,
    pub rank_draws: u32,
    pub rank_bins: u32,
    pub quantities: Vec<ThomasSbcQuantity>,
    pub replicate_diagnostics: Vec<ThomasSbcReplicate>,
    pub resources: ThomasSbcResources,
    pub claim_status: ClaimStatus,
    pub backend: WorkerBackend,
    pub request_sha256: String,
}

/// The ranked quantities, in reported order.
pub const THOMAS_SBC_QUANTITIES: [&str; 9] = [
    "population_mean_log_kappa",
    "population_mean_log_mu",
    "population_mean_log_sigma",
    "between_pattern_sd_log_kappa",
    "between_pattern_sd_log_mu",
    "between_pattern_sd_log_sigma",
    "first_pattern_log_kappa",
    "first_pattern_log_mu",
    "first_pattern_log_sigma",
];

impl ThomasClusterSbcResult {
    /// Check identity, recompute each rank-uniformity p-value from its histogram, and check
    /// that the calibration state and claim follow from the declared policy.
    pub fn validate(
        &self,
        request: &ThomasClusterRequest<ThomasSbcPayload>,
        request_sha256: &str,
    ) -> Result<(), WorkerError> {
        let spec = &request.payload.sbc;
        if !same_identity(
            &self.format,
            self.version,
            &self.backend,
            &self.request_sha256,
            (THOMAS_SBC_RESULT_FORMAT, &request.backend, request_sha256),
        ) || self.model.dilation_um != request.model.dilation_um
            || self.replicates != spec.replicates
            || self.rank_draws != spec.rank_draws
            || self.rank_bins != spec.rank_bins
            || self.replicate_diagnostics.len() != spec.replicates as usize
            || self.resources.work_units != request.checked_budget.0
            || self.resources.kernel_bytes != request.checked_budget.1
            || self
                .quantities
                .iter()
                .map(|row| row.quantity.as_str())
                .ne(THOMAS_SBC_QUANTITIES)
        {
            return Err(contract("Thomas SBC result identity or structure mismatch"));
        }
        let [low, high] = spec.coverage_90_bounds;
        let mut passes = true;
        for row in &self.quantities {
            let (_, p_value) = rank_uniformity(&row.rank_histogram, spec.rank_draws)
                .ok_or_else(|| contract("Thomas SBC rank histogram is invalid"))?;
            if row.rank_histogram.len() != spec.rank_bins as usize
                || row
                    .rank_histogram
                    .iter()
                    .map(|&count| u64::from(count))
                    .sum::<u64>()
                    != u64::from(spec.replicates)
                || (p_value - row.rank_uniformity_p_value).abs()
                    > P_VALUE_TOLERANCE * p_value.max(1e-300).max(1.0)
                || !probability(row.coverage_90)
                || !probability(row.mean_normalized_rank)
            {
                return Err(contract("Thomas SBC rank summary is inconsistent"));
            }
            passes &= spec.minimum_rank_p_value <= row.rank_uniformity_p_value
                && (low..=high).contains(&row.coverage_90);
        }
        let mut nonconverged = 0_u32;
        for (index, row) in self.replicate_diagnostics.iter().enumerate() {
            if row.replicate as usize != index
                || row.point_counts.len() != spec.patterns_per_replicate as usize
                || !row.point_counts.iter().all(|&count| {
                    (spec.minimum_points as usize..=spec.maximum_points as usize).contains(&count)
                })
            {
                return Err(contract(
                    "Thomas SBC replicate diagnostics are inconsistent",
                ));
            }
            passes &= row.parent_capacity_hits == 0;
            nonconverged += u32::from(!row.converged);
        }
        let (state, claim) = if passes {
            (
                "passed",
                ClaimStatus::SyntheticSbcDiagnosticNotBiologicalCalibration,
            )
        } else {
            ("failed", ClaimStatus::DiagnosticOnlyFailedCalibration)
        };
        if self.calibration_state != state
            || self.claim_status != claim
            || self.nonconverged_replicates != nonconverged
        {
            return Err(contract(
                "Thomas SBC calibration state or claim does not follow from the policy",
            ));
        }
        Ok(())
    }
}

/// A `bayes advanced-cluster` output of either published format.
#[derive(Debug)]
pub enum ClusterModelDocument {
    /// `marklab.bayesian_thomas_cluster_model` version 1.
    BayesianThomas(Box<ThomasClusterResult>),
    /// `marklab.advanced_cluster_and_gibbs_models` version 2: k-means heuristic, finite
    /// binary Gibbs exchange fit and fixed shrinkage. It is read, no longer produced.
    LegacyHeuristic(Box<AdvancedClusterResult>),
}

impl ClusterModelDocument {
    /// Decode by the document's `format` and `version` fields.
    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct Identity {
            format: String,
            version: u32,
        }
        let identity: Identity = serde_json::from_slice(bytes)?;
        match (identity.format.as_str(), identity.version) {
            (THOMAS_RESULT_FORMAT, 1) => {
                serde_json::from_slice(bytes).map(|result| Self::BayesianThomas(Box::new(result)))
            }
            (LEGACY_CLUSTER_FORMAT, 2) => {
                serde_json::from_slice(bytes).map(|result| Self::LegacyHeuristic(Box::new(result)))
            }
            (format, version) => Err(<serde_json::Error as serde::de::Error>::custom(format!(
                "unsupported cluster model document {format} version {version}"
            ))),
        }
    }
}

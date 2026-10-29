//! Fused unbalanced Gromov-Wasserstein (FUGW) with KL-relaxed marginals through pinned POT.
//!
//! POT 0.9.7.post1 `ot.gromov.fused_unbalanced_gromov_wasserstein` minimizes the lower bound
//! of FUGW (Thual et al. 2022; Sejourne, Vialard and Peyre 2021) over two couplings `P` and `Q`
//! of equal mass:
//!
//! `sum_ijkl (S_ik - T_jl)^2 P_ij Q_kl + (a / 2) (<C, P> + <C, Q>)
//!  + rho KL(P_1 Q_1^T | u u^T) + rho KL(P_2 Q_2^T | v v^T)
//!  + epsilon KL(P (x) Q | (u v^T) (x) (u v^T))`,
//!
//! where `S` and `T` are the scaled structure matrices, `C` the squared Euclidean feature cost,
//! `u` and `v` the source and target masses and `KL(x | y) = sum x log(x / y) - x + y`. Unlike
//! partial transport, marginals may fall below or exceed the supplied masses; `rho` sets how
//! strongly they are held to them. The worker fits from three starting plans and reports the
//! lowest objective with the spread across starts.

use marklab_core::backend::is_worker_timeout;
use marklab_core::backend::versions::{POT_VERSION, PYTHON_VERSION};
use marklab_core::backend::{BackendContract, ClaimStatus, FitState, WorkerBackend, WorkerError};
use serde::{Deserialize, Serialize};

use crate::fgw_common::{
    approximately_equal, audit_plan, same_values, valid_structure, valid_support,
};
use crate::{FgwPlanEntry, FgwSupport};

const MAXIMUM_POINTS: usize = 32;
const MAXIMUM_FEATURE_DIMENSION: usize = 128;
/// Upper bound on structure-tensor evaluations: three starts, each BCD iteration contracting
/// the `n^2 m^2` square-loss tensor with both couplings.
const MAXIMUM_TENSOR_WORK: u64 = 2_000_000_000;
const INITIALIZATIONS: [&str; 3] = ["independent_mass", "feature_emd", "structure_profile_emd"];

#[derive(Clone, Debug)]
pub struct UnbalancedFusedGromovWassersteinSpec {
    pub source: Vec<FgwSupport>,
    pub target: Vec<FgwSupport>,
    pub source_structure_row_major: Vec<f64>,
    pub target_structure_row_major: Vec<f64>,
    /// `a`, the weight of the linear feature term.
    pub linear_weight: f64,
    /// `rho`, the KL marginal relaxation strength.
    pub marginal_relaxation: f64,
    /// Entropic regularization; zero selects POT's KL majorization-minimization solver.
    pub epsilon: f64,
    pub feature_scale: f64,
    pub structure_scale: f64,
    pub tolerance: f64,
    pub inner_tolerance: f64,
    pub maximum_iterations: u32,
    pub maximum_inner_iterations: u32,
    pub timeout_seconds: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct UnbalancedFgwResources {
    pub maximum_source_points: u32,
    pub maximum_target_points: u32,
    pub maximum_feature_dimension: u32,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnbalancedFusedGromovWassersteinWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub source: Vec<FgwSupport>,
    pub target: Vec<FgwSupport>,
    pub source_structure_row_major: Vec<f64>,
    pub target_structure_row_major: Vec<f64>,
    pub linear_weight: f64,
    pub marginal_relaxation: f64,
    pub epsilon: f64,
    pub feature_scale: f64,
    pub structure_scale: f64,
    pub tolerance: f64,
    pub maximum_iterations: u32,
    pub inner_tolerance: f64,
    pub maximum_inner_iterations: u32,
    pub resources: UnbalancedFgwResources,
}

impl UnbalancedFusedGromovWassersteinWorkerRequest {
    pub fn new(
        spec: UnbalancedFusedGromovWassersteinSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, WorkerError> {
        validate_spec(&spec)?;
        Ok(Self {
            format: "marklab.pot_unbalanced_fused_gromov_wasserstein_request",
            version: 1,
            backend: BackendContract {
                name: "pot",
                version: POT_VERSION,
                python_version: PYTHON_VERSION,
                environment_lock_sha256,
                worker_sha256,
            },
            source: spec.source,
            target: spec.target,
            source_structure_row_major: spec.source_structure_row_major,
            target_structure_row_major: spec.target_structure_row_major,
            linear_weight: spec.linear_weight,
            marginal_relaxation: spec.marginal_relaxation,
            epsilon: spec.epsilon,
            feature_scale: spec.feature_scale,
            structure_scale: spec.structure_scale,
            tolerance: spec.tolerance,
            maximum_iterations: spec.maximum_iterations,
            inner_tolerance: spec.inner_tolerance,
            maximum_inner_iterations: spec.maximum_inner_iterations,
            resources: UnbalancedFgwResources {
                maximum_source_points: MAXIMUM_POINTS as u32,
                maximum_target_points: MAXIMUM_POINTS as u32,
                maximum_feature_dimension: MAXIMUM_FEATURE_DIMENSION as u32,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

fn finite_in(value: f64, low: f64, high: f64) -> bool {
    value.is_finite() && low <= value && value <= high
}

fn validate_spec(spec: &UnbalancedFusedGromovWassersteinSpec) -> Result<(), WorkerError> {
    let rows = spec.source.len();
    let columns = spec.target.len();
    let dimension = spec.source.first().map_or(0, |row| row.features.len());
    if !((1..=MAXIMUM_POINTS).contains(&rows)
        && (1..=MAXIMUM_POINTS).contains(&columns)
        && (1..=MAXIMUM_FEATURE_DIMENSION).contains(&dimension)
        && spec.source_structure_row_major.len() == rows * rows
        && spec.target_structure_row_major.len() == columns * columns
        && finite_in(spec.linear_weight, 0.0, 1e6)
        && finite_in(spec.marginal_relaxation, 1e-6, 1e6)
        && finite_in(spec.epsilon, 0.0, 1e3)
        && finite_in(spec.feature_scale, f64::MIN_POSITIVE, f64::MAX)
        && finite_in(spec.structure_scale, f64::MIN_POSITIVE, f64::MAX)
        && finite_in(spec.tolerance, 1e-15, 1.0)
        && finite_in(spec.inner_tolerance, 1e-15, 1.0)
        && (1..=10_000).contains(&spec.maximum_iterations)
        && (1..=100_000).contains(&spec.maximum_inner_iterations)
        && is_worker_timeout(spec.timeout_seconds)
        && valid_support(&spec.source, dimension, |mass| {
            (f64::MIN_POSITIVE..=1e6).contains(&mass)
        })
        && valid_support(&spec.target, dimension, |mass| {
            (f64::MIN_POSITIVE..=1e6).contains(&mass)
        }))
    {
        return Err(WorkerError::InvalidSpec(
            "unbalanced FGW dimensions, supports, or controls are invalid".into(),
        ));
    }
    for (matrix, size, label) in [
        (&spec.source_structure_row_major, rows, "source"),
        (&spec.target_structure_row_major, columns, "target"),
    ] {
        if !valid_structure(matrix, size) {
            return Err(WorkerError::InvalidSpec(format!(
                "unbalanced FGW {label} structure must be finite, nonnegative, symmetric and zero on the diagonal"
            )));
        }
    }
    let tensor_work = (rows * rows * columns * columns) as u64
        * u64::from(spec.maximum_iterations)
        * INITIALIZATIONS.len() as u64;
    if tensor_work > MAXIMUM_TENSOR_WORK {
        return Err(WorkerError::InvalidSpec(
            "unbalanced FGW structure-tensor work exceeds its bound".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnbalancedFgwObjective {
    pub structural: f64,
    pub linear: f64,
    pub source_marginal_kl: f64,
    pub target_marginal_kl: f64,
    pub entropic_kl: f64,
    pub total: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnbalancedFgwFit {
    pub initialization: String,
    pub iterations: u32,
    pub final_plan_change: f64,
    pub converged: bool,
    pub plan: Vec<FgwPlanEntry>,
    /// Largest entry of `|P - Q|`; the lower bound need not make the two couplings equal.
    pub coupling_difference_maximum: f64,
    pub source_marginals: Vec<f64>,
    pub target_marginals: Vec<f64>,
    pub transported_mass: f64,
    pub source_marginal_minus_mass: Vec<f64>,
    pub target_marginal_minus_mass: Vec<f64>,
    pub objective: UnbalancedFgwObjective,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnbalancedFusedGromovWassersteinWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub best_initialization: String,
    pub initialization_sensitivity: Vec<UnbalancedFgwFit>,
    pub objective_spread: f64,
    pub claim_status: ClaimStatus,
}

fn contract(message: &str) -> WorkerError {
    WorkerError::WorkerContract(message.into())
}

impl UnbalancedFusedGromovWassersteinWorkerResult {
    /// Replay plan identities, feature costs, marginals and the objective total, and check that
    /// the best start, fit state and objective spread follow from the reported fits.
    pub fn validate(
        &self,
        request: &UnbalancedFusedGromovWassersteinWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), WorkerError> {
        if self.format != "marklab.pot_unbalanced_fused_gromov_wasserstein_worker_result"
            || self.version != 1
            || !self.backend.matches(&request.backend)
            || self.request_sha256 != request_sha256
            || self.claim_status != ClaimStatus::EnsembleDescriptiveAlignmentNotCorrespondence
            || self
                .initialization_sensitivity
                .iter()
                .map(|fit| fit.initialization.as_str())
                .ne(INITIALIZATIONS)
        {
            return Err(contract(
                "unbalanced FGW worker identity or initializations differ",
            ));
        }
        for fit in &self.initialization_sensitivity {
            validate_fit(fit, request)?;
        }
        let best = self.best();
        let worst = self
            .initialization_sensitivity
            .iter()
            .map(|fit| fit.objective.total)
            .fold(f64::NEG_INFINITY, f64::max);
        let state = if best.converged {
            FitState::Complete
        } else {
            FitState::Nonconverged
        };
        if self.best_initialization != best.initialization
            || self.fit_state != state
            || !approximately_equal(self.objective_spread, worst - best.objective.total)
        {
            return Err(contract(
                "unbalanced FGW best start, fit state, or objective spread differs",
            ));
        }
        Ok(())
    }

    /// The fit with the lowest objective; ties keep the earlier start.
    pub fn best(&self) -> &UnbalancedFgwFit {
        self.initialization_sensitivity
            .iter()
            .reduce(|best, fit| {
                if fit.objective.total < best.objective.total {
                    fit
                } else {
                    best
                }
            })
            .expect("three initialization fits exist")
    }
}

fn validate_fit(
    fit: &UnbalancedFgwFit,
    request: &UnbalancedFusedGromovWassersteinWorkerRequest,
) -> Result<(), WorkerError> {
    let rows = request.source.len();
    let columns = request.target.len();
    if fit.plan.len() != rows * columns
        || fit.source_marginals.len() != rows
        || fit.target_marginals.len() != columns
        || fit.source_marginal_minus_mass.len() != rows
        || fit.target_marginal_minus_mass.len() != columns
        || fit.iterations == 0
        || fit.iterations > request.maximum_iterations
        || !finite_in(fit.final_plan_change, 0.0, f64::MAX)
        || !finite_in(fit.coupling_difference_maximum, 0.0, f64::MAX)
        || fit.converged != (fit.final_plan_change < request.tolerance)
    {
        return Err(contract(
            "unbalanced FGW fit dimensions or convergence differ",
        ));
    }
    let marginals = audit_plan(
        &fit.plan,
        &request.source,
        &request.target,
        request.feature_scale,
        |_, _| {},
    )
    .ok_or_else(|| contract("unbalanced FGW plan entry differs"))?;
    let source_marginals = marginals.source;
    let target_marginals = marginals.target;
    let deviations = |marginals: &[f64], supports: &[FgwSupport]| {
        marginals
            .iter()
            .zip(supports)
            .map(|(marginal, support)| marginal - support.mass)
            .collect::<Vec<_>>()
    };
    let objective = &fit.objective;
    let total = objective.structural
        + objective.linear
        + request.marginal_relaxation
            * (objective.source_marginal_kl + objective.target_marginal_kl)
        + request.epsilon * objective.entropic_kl;
    let terms = [
        objective.structural,
        objective.linear,
        objective.source_marginal_kl,
        objective.target_marginal_kl,
        objective.entropic_kl,
    ];
    if !same_values(&source_marginals, &fit.source_marginals)
        || !same_values(&target_marginals, &fit.target_marginals)
        || !approximately_equal(source_marginals.iter().sum(), fit.transported_mass)
        || !same_values(
            &deviations(&source_marginals, &request.source),
            &fit.source_marginal_minus_mass,
        )
        || !same_values(
            &deviations(&target_marginals, &request.target),
            &fit.target_marginal_minus_mass,
        )
        || !terms.iter().all(|term| term.is_finite() && *term >= -1e-12)
        || (request.epsilon == 0.0 && objective.entropic_kl != 0.0)
        || !approximately_equal(total, objective.total)
    {
        return Err(contract(
            "unbalanced FGW marginals or objective replay differs",
        ));
    }
    Ok(())
}

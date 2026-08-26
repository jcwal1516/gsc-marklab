use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{BackendContract, BayesError, FitState, WorkerBackend};

const POT_VERSION: &str = "0.9.7.post1";
const MAXIMUM_POINTS: usize = 32;
const MARGINAL_TOLERANCE: f64 = 1e-7;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FgwSupport {
    pub id: String,
    pub mass: f64,
    pub features: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct FusedGromovWassersteinSpec {
    pub source: Vec<FgwSupport>,
    pub target: Vec<FgwSupport>,
    pub source_structure_row_major: Vec<f64>,
    pub target_structure_row_major: Vec<f64>,
    pub alpha: f64,
    pub epsilon: f64,
    pub feature_scale: f64,
    pub structure_scale: f64,
    pub tolerance: f64,
    pub maximum_iterations: u32,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FusedGromovWassersteinWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub source: Vec<FgwSupport>,
    pub target: Vec<FgwSupport>,
    pub source_structure_row_major: Vec<f64>,
    pub target_structure_row_major: Vec<f64>,
    pub alpha: f64,
    pub epsilon: f64,
    pub feature_scale: f64,
    pub structure_scale: f64,
    pub tolerance: f64,
    pub maximum_iterations: u32,
    pub resources: FgwResources,
}

#[derive(Clone, Debug, Serialize)]
pub struct FgwResources {
    pub maximum_source_points: u32,
    pub maximum_target_points: u32,
    pub maximum_feature_dimension: u32,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

impl FusedGromovWassersteinWorkerRequest {
    pub fn new(
        spec: FusedGromovWassersteinSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        validate_spec(&spec)?;
        Ok(Self {
            format: "marklab.pot_fused_gromov_wasserstein_request",
            version: 1,
            backend: BackendContract {
                name: "pot",
                version: POT_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            source: spec.source,
            target: spec.target,
            source_structure_row_major: spec.source_structure_row_major,
            target_structure_row_major: spec.target_structure_row_major,
            alpha: spec.alpha,
            epsilon: spec.epsilon,
            feature_scale: spec.feature_scale,
            structure_scale: spec.structure_scale,
            tolerance: spec.tolerance,
            maximum_iterations: spec.maximum_iterations,
            resources: FgwResources {
                maximum_source_points: MAXIMUM_POINTS as u32,
                maximum_target_points: MAXIMUM_POINTS as u32,
                maximum_feature_dimension: 128,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

fn validate_spec(spec: &FusedGromovWassersteinSpec) -> Result<(), BayesError> {
    let rows = spec.source.len();
    let columns = spec.target.len();
    if !((1..=MAXIMUM_POINTS).contains(&rows)
        && (1..=MAXIMUM_POINTS).contains(&columns)
        && spec.source_structure_row_major.len() == rows * rows
        && spec.target_structure_row_major.len() == columns * columns
        && spec.alpha.is_finite()
        && (0.0..=1.0).contains(&spec.alpha)
        && spec.epsilon.is_finite()
        && spec.epsilon > 0.0
        && spec.feature_scale.is_finite()
        && spec.feature_scale > 0.0
        && spec.structure_scale.is_finite()
        && spec.structure_scale > 0.0
        && spec.tolerance.is_finite()
        && spec.tolerance > 0.0
        && spec.tolerance <= 1.0
        && (1..=10_000).contains(&spec.maximum_iterations)
        && (1..=3_600).contains(&spec.timeout_seconds))
    {
        return Err(BayesError::InvalidSpec(
            "FGW dimensions or controls are invalid".into(),
        ));
    }
    let feature_dimension = spec.source.first().map_or(0, |row| row.features.len());
    if !(1..=128).contains(&feature_dimension)
        || !validate_support(&spec.source, feature_dimension)
        || !validate_support(&spec.target, feature_dimension)
    {
        return Err(BayesError::InvalidSpec(
            "FGW support identities, masses, or features are invalid".into(),
        ));
    }
    let source_total = spec.source.iter().map(|row| row.mass).sum::<f64>();
    let target_total = spec.target.iter().map(|row| row.mass).sum::<f64>();
    if !approximately_equal(source_total, 1.0) || !approximately_equal(target_total, 1.0) {
        return Err(BayesError::InvalidSpec(
            "FGW masses must each sum to one".into(),
        ));
    }
    validate_structure(&spec.source_structure_row_major, rows, "source")?;
    validate_structure(&spec.target_structure_row_major, columns, "target")?;
    let solver_work = rows as u64 * columns as u64 * u64::from(spec.maximum_iterations) * 3;
    let replay_work = (rows * rows * columns * columns) as u64 * 3;
    if solver_work > 50_000_000 || replay_work > 50_000_000 {
        return Err(BayesError::InvalidSpec("FGW work exceeds its bound".into()));
    }
    Ok(())
}

fn validate_support(rows: &[FgwSupport], feature_dimension: usize) -> bool {
    let mut ids = HashSet::new();
    rows.iter().all(|row| {
        !row.id.trim().is_empty()
            && row.id.trim() == row.id
            && ids.insert(row.id.as_str())
            && row.mass.is_finite()
            && row.mass > 0.0
            && row.features.len() == feature_dimension
            && row.features.iter().all(|value| value.is_finite())
    })
}

fn validate_structure(matrix: &[f64], size: usize, label: &str) -> Result<(), BayesError> {
    for row in 0..size {
        for column in 0..size {
            let value = matrix[row * size + column];
            if !value.is_finite()
                || value < 0.0
                || (row == column && value.abs() > 1e-12)
                || !approximately_equal(value, matrix[column * size + row])
            {
                return Err(BayesError::InvalidSpec(format!(
                    "FGW {label} structure must be finite, nonnegative, symmetric, and zero-diagonal"
                )));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FgwPlanEntry {
    pub source_id: String,
    pub target_id: String,
    pub mass: f64,
    pub feature_cost: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FgwInitializationResult {
    pub initialization: String,
    pub iterations: u32,
    pub converged: bool,
    pub final_plan_change: f64,
    pub plan: Vec<FgwPlanEntry>,
    pub source_marginals: Vec<f64>,
    pub target_marginals: Vec<f64>,
    pub maximum_marginal_residual: f64,
    pub feature_objective: f64,
    pub structural_objective: f64,
    pub entropy: f64,
    pub regularized_objective: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FusedGromovWassersteinWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub best_initialization: String,
    pub best_plan: Vec<FgwPlanEntry>,
    pub initialization_sensitivity: Vec<FgwInitializationResult>,
    pub claim_status: String,
}

impl FusedGromovWassersteinWorkerResult {
    pub fn validate(
        &self,
        request: &FusedGromovWassersteinWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.pot_fused_gromov_wasserstein_worker_result"
            || self.version != 1
            || self.backend.name != "pot"
            || self.backend.version != POT_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.claim_status != "descriptive_alignment_not_correspondence"
            || self.initialization_sensitivity.len() != 3
        {
            return Err(BayesError::WorkerContract(
                "FGW worker identity or sensitivity dimensions differ".into(),
            ));
        }
        let expected_initializations = ["independent_mass", "feature_emd", "structure_profile_emd"];
        for (fit, expected) in self
            .initialization_sensitivity
            .iter()
            .zip(expected_initializations)
        {
            if fit.initialization != expected {
                return Err(BayesError::WorkerContract(
                    "FGW initialization order differs".into(),
                ));
            }
            validate_fit(fit, request)?;
        }
        let best = self
            .initialization_sensitivity
            .iter()
            .min_by(|left, right| {
                left.regularized_objective
                    .total_cmp(&right.regularized_objective)
            })
            .expect("three initialization fits exist");
        let expected_state = if self
            .initialization_sensitivity
            .iter()
            .all(|fit| fit.converged)
        {
            FitState::Complete
        } else {
            FitState::Nonconverged
        };
        if self.fit_state != expected_state
            || self.best_initialization != best.initialization
            || !same_plan(&self.best_plan, &best.plan)
        {
            return Err(BayesError::WorkerContract(
                "FGW best-plan selection or fit state differs".into(),
            ));
        }
        Ok(())
    }
}

fn validate_fit(
    fit: &FgwInitializationResult,
    request: &FusedGromovWassersteinWorkerRequest,
) -> Result<(), BayesError> {
    let rows = request.source.len();
    let columns = request.target.len();
    if fit.plan.len() != rows * columns
        || fit.source_marginals.len() != rows
        || fit.target_marginals.len() != columns
        || fit.iterations == 0
        || fit.iterations > request.maximum_iterations
        || !fit.final_plan_change.is_finite()
        || fit.final_plan_change < 0.0
    {
        return Err(BayesError::WorkerContract(
            "FGW fit dimensions or diagnostics differ".into(),
        ));
    }
    let mut source_marginals = vec![0.0; rows];
    let mut target_marginals = vec![0.0; columns];
    let mut feature_objective = 0.0;
    let mut entropy = 0.0;
    for (row, source_marginal) in source_marginals.iter_mut().enumerate() {
        for (column, target_marginal) in target_marginals.iter_mut().enumerate() {
            let entry = &fit.plan[row * columns + column];
            let feature_cost = squared_feature_cost(
                &request.source[row].features,
                &request.target[column].features,
                request.feature_scale,
            );
            if entry.source_id != request.source[row].id
                || entry.target_id != request.target[column].id
                || !approximately_equal(entry.feature_cost, feature_cost)
                || !entry.mass.is_finite()
                || entry.mass < 0.0
            {
                return Err(BayesError::WorkerContract(
                    "FGW plan identity, feature cost, or mass differs".into(),
                ));
            }
            *source_marginal += entry.mass;
            *target_marginal += entry.mass;
            feature_objective += entry.mass * feature_cost;
            if entry.mass > 0.0 {
                entropy -= entry.mass * entry.mass.ln();
            }
        }
    }
    let structural_objective = structural_objective(&fit.plan, request);
    let regularized_objective = request.alpha * feature_objective
        + (1.0 - request.alpha) * structural_objective
        - request.epsilon * entropy;
    let residual = source_marginals
        .iter()
        .zip(&request.source)
        .map(|(actual, expected)| (actual - expected.mass).abs())
        .chain(
            target_marginals
                .iter()
                .zip(&request.target)
                .map(|(actual, expected)| (actual - expected.mass).abs()),
        )
        .fold(0.0, f64::max);
    if source_marginals
        .iter()
        .zip(&fit.source_marginals)
        .any(|(left, right)| !approximately_equal(*left, *right))
        || target_marginals
            .iter()
            .zip(&fit.target_marginals)
            .any(|(left, right)| !approximately_equal(*left, *right))
        || !approximately_equal(residual, fit.maximum_marginal_residual)
        || !approximately_equal(feature_objective, fit.feature_objective)
        || !approximately_equal(structural_objective, fit.structural_objective)
        || !approximately_equal(entropy, fit.entropy)
        || !approximately_equal(regularized_objective, fit.regularized_objective)
        || residual > MARGINAL_TOLERANCE
        || fit.converged != (fit.final_plan_change <= request.tolerance)
    {
        return Err(BayesError::WorkerContract(
            "FGW marginal, objective, or convergence replay differs".into(),
        ));
    }
    Ok(())
}

fn squared_feature_cost(source: &[f64], target: &[f64], scale: f64) -> f64 {
    source
        .iter()
        .zip(target)
        .map(|(left, right)| ((left - right) / scale).powi(2))
        .sum()
}

fn structural_objective(
    plan: &[FgwPlanEntry],
    request: &FusedGromovWassersteinWorkerRequest,
) -> f64 {
    let rows = request.source.len();
    let columns = request.target.len();
    let mut objective = 0.0;
    for source_left in 0..rows {
        for target_left in 0..columns {
            for source_right in 0..rows {
                for target_right in 0..columns {
                    let source_distance = request.source_structure_row_major
                        [source_left * rows + source_right]
                        / request.structure_scale;
                    let target_distance = request.target_structure_row_major
                        [target_left * columns + target_right]
                        / request.structure_scale;
                    objective += (source_distance - target_distance).powi(2)
                        * plan[source_left * columns + target_left].mass
                        * plan[source_right * columns + target_right].mass;
                }
            }
        }
    }
    objective
}

fn same_plan(left: &[FgwPlanEntry], right: &[FgwPlanEntry]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.source_id == right.source_id
                && left.target_id == right.target_id
                && approximately_equal(left.mass, right.mass)
                && approximately_equal(left.feature_cost, right.feature_cost)
        })
}

fn approximately_equal(left: f64, right: f64) -> bool {
    left.is_finite()
        && right.is_finite()
        && (left - right).abs() <= 1e-8 * (1.0 + left.abs().max(right.abs()))
}

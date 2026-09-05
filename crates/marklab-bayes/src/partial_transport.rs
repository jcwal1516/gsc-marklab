use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{BackendContract, BayesError, TransportMass, WorkerBackend};

const SCIPY_VERSION: &str = "1.18.1";
const FEASIBILITY_TOLERANCE: f64 = 1e-8;

#[derive(Clone, Debug)]
pub struct PartialTransportSpec {
    pub source: Vec<TransportMass>,
    pub target: Vec<TransportMass>,
    pub costs_row_major: Vec<f64>,
    pub transported_mass: f64,
    pub epsilon: f64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PartialTransportWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub source: Vec<TransportMass>,
    pub target: Vec<TransportMass>,
    pub costs_row_major: Vec<f64>,
    pub transported_mass: f64,
    pub epsilon: f64,
    pub resources: PartialTransportResources,
}

#[derive(Clone, Debug, Serialize)]
pub struct PartialTransportResources {
    pub maximum_source_points: u32,
    pub maximum_target_points: u32,
    pub maximum_plan_variables: u32,
    pub feasibility_tolerance: f64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

impl PartialTransportWorkerRequest {
    pub fn new(
        spec: PartialTransportSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !((1..=64).contains(&spec.source.len())
            && (1..=64).contains(&spec.target.len())
            && spec.costs_row_major.len() == spec.source.len() * spec.target.len()
            && spec.transported_mass.is_finite()
            && spec.transported_mass > 0.0
            && spec.epsilon.is_finite()
            && spec.epsilon > 0.0
            && (1..=3_600).contains(&spec.timeout_seconds))
        {
            return Err(BayesError::InvalidSpec(
                "partial transport dimensions or controls are invalid".into(),
            ));
        }
        validate_support(&spec.source, "source")?;
        validate_support(&spec.target, "target")?;
        if spec
            .costs_row_major
            .iter()
            .any(|cost| !cost.is_finite() || *cost < 0.0)
        {
            return Err(BayesError::InvalidSpec(
                "partial transport costs must be finite and nonnegative".into(),
            ));
        }
        let source_total = spec.source.iter().map(|row| row.mass).sum::<f64>();
        let target_total = spec.target.iter().map(|row| row.mass).sum::<f64>();
        if spec.transported_mass > source_total.min(target_total) {
            return Err(BayesError::InvalidSpec(
                "partial transported mass exceeds available mass".into(),
            ));
        }
        Ok(Self {
            format: "marklab.scipy_partial_transport_request",
            version: 1,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            source: spec.source,
            target: spec.target,
            costs_row_major: spec.costs_row_major,
            transported_mass: spec.transported_mass,
            epsilon: spec.epsilon,
            resources: PartialTransportResources {
                maximum_source_points: 64,
                maximum_target_points: 64,
                maximum_plan_variables: 4_096,
                feasibility_tolerance: FEASIBILITY_TOLERANCE,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

fn validate_support(rows: &[TransportMass], label: &str) -> Result<(), BayesError> {
    let mut ids = HashSet::new();
    if rows.iter().any(|row| {
        row.id.trim().is_empty()
            || row.id.trim() != row.id
            || !ids.insert(row.id.as_str())
            || !row.mass.is_finite()
            || row.mass < 0.0
    }) || rows.iter().all(|row| row.mass == 0.0)
    {
        return Err(BayesError::InvalidSpec(format!(
            "partial transport {label} support is invalid"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PartialTransportPlanEntry {
    pub source_id: String,
    pub target_id: String,
    pub mass: f64,
    pub cost: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PartialTransportOptimizer {
    pub method: String,
    pub success: bool,
    pub iterations: u32,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialTransportWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub optimizer: PartialTransportOptimizer,
    pub plan: Vec<PartialTransportPlanEntry>,
    pub source_marginals: Vec<f64>,
    pub target_marginals: Vec<f64>,
    pub transported_mass: f64,
    pub unmatched_source_mass: Vec<f64>,
    pub unmatched_target_mass: Vec<f64>,
    pub transport_cost: f64,
    pub entropy: f64,
    pub regularized_objective: f64,
    pub maximum_constraint_violation: f64,
    pub constraint_status: String,
}

impl PartialTransportWorkerResult {
    pub fn validate(
        &self,
        request: &PartialTransportWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        let rows = request.source.len();
        let columns = request.target.len();
        if self.format != "marklab.scipy_partial_transport_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.optimizer.method != "SLSQP"
            || !self.optimizer.success
            || self.plan.len() != rows * columns
            || self.source_marginals.len() != rows
            || self.target_marginals.len() != columns
            || self.unmatched_source_mass.len() != rows
            || self.unmatched_target_mass.len() != columns
            || self.constraint_status != "feasible_within_tolerance"
        {
            return Err(BayesError::WorkerContract(
                "partial transport worker identity or dimensions differ".into(),
            ));
        }
        let mut source_marginals = vec![0.0; rows];
        let mut target_marginals = vec![0.0; columns];
        let mut transport_cost = 0.0;
        let mut entropy = 0.0;
        for (row, (source_marginal, plan_row)) in source_marginals
            .iter_mut()
            .zip(self.plan.chunks_exact(columns))
            .enumerate()
        {
            for (column, (target_marginal, output)) in
                target_marginals.iter_mut().zip(plan_row).enumerate()
            {
                let index = row * columns + column;
                let cost = request.costs_row_major[index];
                if output.source_id != request.source[row].id
                    || output.target_id != request.target[column].id
                    || output.cost.to_bits() != cost.to_bits()
                    || !output.mass.is_finite()
                    || output.mass < 0.0
                {
                    return Err(BayesError::WorkerContract(
                        "partial transport plan entry differs".into(),
                    ));
                }
                *source_marginal += output.mass;
                *target_marginal += output.mass;
                transport_cost += output.mass * cost;
                if output.mass > 0.0 {
                    entropy -= output.mass * output.mass.ln();
                }
            }
        }
        let transported_mass = source_marginals.iter().sum::<f64>();
        let regularized_objective = transport_cost
            + request.epsilon
                * self
                    .plan
                    .iter()
                    .filter(|entry| entry.mass > 0.0)
                    .map(|entry| entry.mass * (entry.mass.ln() - 1.0))
                    .sum::<f64>();
        let mut violation: f64 = (transported_mass - request.transported_mass).abs();
        for (actual, expected) in source_marginals.iter().zip(&request.source) {
            violation = violation.max((actual - expected.mass).max(0.0));
        }
        for (actual, expected) in target_marginals.iter().zip(&request.target) {
            violation = violation.max((actual - expected.mass).max(0.0));
        }
        if source_marginals
            .iter()
            .zip(&self.source_marginals)
            .any(|(left, right)| !approximately_equal(*left, *right))
            || target_marginals
                .iter()
                .zip(&self.target_marginals)
                .any(|(left, right)| !approximately_equal(*left, *right))
            || !approximately_equal(self.transported_mass, transported_mass)
            || !approximately_equal(self.transport_cost, transport_cost)
            || !approximately_equal(self.entropy, entropy)
            || !approximately_equal(self.regularized_objective, regularized_objective)
            || !approximately_equal(self.maximum_constraint_violation, violation)
            || violation > FEASIBILITY_TOLERANCE
        {
            return Err(BayesError::WorkerContract(
                "partial transport feasibility or objective differs".into(),
            ));
        }
        for ((capacity, actual), unmatched) in request
            .source
            .iter()
            .zip(&source_marginals)
            .zip(&self.unmatched_source_mass)
        {
            if !approximately_equal(*unmatched, capacity.mass - actual) {
                return Err(BayesError::WorkerContract(
                    "partial unmatched source mass differs".into(),
                ));
            }
        }
        for ((capacity, actual), unmatched) in request
            .target
            .iter()
            .zip(&target_marginals)
            .zip(&self.unmatched_target_mass)
        {
            if !approximately_equal(*unmatched, capacity.mass - actual) {
                return Err(BayesError::WorkerContract(
                    "partial unmatched target mass differs".into(),
                ));
            }
        }
        Ok(())
    }
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-8 * (1.0 + left.abs().max(right.abs()))
}

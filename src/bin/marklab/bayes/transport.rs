use std::{collections::BTreeMap, fs, path::PathBuf};

use marklab_bayes::{
    entropic_soft_assignment, sinkhorn_ot, unbalanced_sinkhorn, EntropicSoftAssignmentSpec,
    PartialTransportSpec, PartialTransportWorkerRequest, PartialTransportWorkerResult,
    SinkhornSpec, TransportError, TransportMass, UnbalancedSinkhornSpec,
};
use serde::{Deserialize, Serialize};

use super::{embedding_spatial, publish_json, run_worker, BayesCliError};

#[derive(Deserialize)]
struct SourceRow {
    source_id: String,
    mass: f64,
}

#[derive(Deserialize)]
struct TargetRow {
    target_id: String,
    mass: f64,
}

#[derive(Deserialize)]
struct CostRow {
    source_id: String,
    target_id: String,
    cost: f64,
}

pub(super) fn run_sinkhorn(
    source: PathBuf,
    target: PathBuf,
    cost: PathBuf,
    epsilon: f64,
    tolerance: f64,
    maximum_iterations: u32,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let source_bytes = embedding_spatial::read(&source)?;
    let target_bytes = embedding_spatial::read(&target)?;
    let cost_bytes = embedding_spatial::read(&cost)?;
    let source_rows = read_source(&source_bytes)?;
    let target_rows = read_target(&target_bytes)?;
    let mut costs = BTreeMap::new();
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(cost_bytes.as_slice());
    if !reader
        .headers()?
        .iter()
        .eq(["source_id", "target_id", "cost"])
    {
        return Err(BayesCliError::Input("transport cost header differs".into()));
    }
    for row in reader.deserialize::<CostRow>() {
        let row = row?;
        if costs
            .insert((row.source_id, row.target_id), row.cost)
            .is_some()
        {
            return Err(BayesCliError::Input("duplicate transport cost pair".into()));
        }
    }
    let costs_row_major = source_rows
        .iter()
        .flat_map(|source_row| {
            target_rows.iter().map(|target_row| {
                costs
                    .get(&(source_row.id.clone(), target_row.id.clone()))
                    .copied()
                    .ok_or_else(|| {
                        BayesCliError::Input("transport cost matrix is incomplete".into())
                    })
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if costs.len() != costs_row_major.len() {
        return Err(BayesCliError::Input(
            "transport cost matrix contains undeclared identities".into(),
        ));
    }
    let result = sinkhorn_ot(SinkhornSpec {
        source: source_rows,
        target: target_rows,
        costs_row_major,
        epsilon,
        tolerance,
        maximum_iterations,
    })
    .map_err(map)?;
    publish_json(
        &out,
        &Output {
            source_sha256: marklab_bayes::sha256_hex(&source_bytes),
            target_sha256: marklab_bayes::sha256_hex(&target_bytes),
            cost_sha256: marklab_bayes::sha256_hex(&cost_bytes),
            result,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_unbalanced(
    source: PathBuf,
    target: PathBuf,
    cost: PathBuf,
    epsilon: f64,
    tau_source: f64,
    tau_target: f64,
    tolerance: f64,
    maximum_iterations: u32,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let source_bytes = embedding_spatial::read(&source)?;
    let target_bytes = embedding_spatial::read(&target)?;
    let cost_bytes = embedding_spatial::read(&cost)?;
    let source_rows = read_source(&source_bytes)?;
    let target_rows = read_target(&target_bytes)?;
    let costs_row_major = read_costs(&cost_bytes, &source_rows, &target_rows)?;
    let result = unbalanced_sinkhorn(UnbalancedSinkhornSpec {
        source: source_rows,
        target: target_rows,
        costs_row_major,
        epsilon,
        tau_source,
        tau_target,
        tolerance,
        maximum_iterations,
    })
    .map_err(map)?;
    publish_json(
        &out,
        &UnbalancedOutput {
            source_sha256: marklab_bayes::sha256_hex(&source_bytes),
            target_sha256: marklab_bayes::sha256_hex(&target_bytes),
            cost_sha256: marklab_bayes::sha256_hex(&cost_bytes),
            result,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_soft_assignment(
    source: PathBuf,
    target: PathBuf,
    cost: PathBuf,
    epsilon: f64,
    dustbin_cost: f64,
    tolerance: f64,
    maximum_iterations: u32,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let source_bytes = embedding_spatial::read(&source)?;
    let target_bytes = embedding_spatial::read(&target)?;
    let cost_bytes = embedding_spatial::read(&cost)?;
    let source_rows = read_source(&source_bytes)?;
    let target_rows = read_target(&target_bytes)?;
    let costs_row_major = read_costs(&cost_bytes, &source_rows, &target_rows)?;
    let result = entropic_soft_assignment(EntropicSoftAssignmentSpec {
        source: source_rows,
        target: target_rows,
        costs_row_major,
        epsilon,
        dustbin_cost,
        tolerance,
        maximum_iterations,
    })
    .map_err(map)?;
    publish_json(
        &out,
        &SoftAssignmentOutput {
            source_sha256: marklab_bayes::sha256_hex(&source_bytes),
            target_sha256: marklab_bayes::sha256_hex(&target_bytes),
            cost_sha256: marklab_bayes::sha256_hex(&cost_bytes),
            result,
        },
    )
}

pub(super) fn run_partial(
    source: PathBuf,
    target: PathBuf,
    cost: PathBuf,
    transported_mass: f64,
    epsilon: f64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let source_bytes = embedding_spatial::read(&source)?;
    let target_bytes = embedding_spatial::read(&target)?;
    let cost_bytes = embedding_spatial::read(&cost)?;
    let source_rows = read_source(&source_bytes)?;
    let target_rows = read_target(&target_bytes)?;
    let costs_row_major = read_costs(&cost_bytes, &source_rows, &target_rows)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_name = "marklab_scipy_partial_transport_worker.py";
    let worker_path = worker_directory.join(worker_name);
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = PartialTransportWorkerRequest::new(
        PartialTransportSpec {
            source: source_rows,
            target: target_rows,
            costs_row_major,
            transported_mass,
            epsilon,
            timeout_seconds,
        },
        marklab_bayes::sha256_hex(&lock_bytes),
        marklab_bayes::sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = marklab_bayes::sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, worker_name, &request_bytes, timeout_seconds)?;
    let result: PartialTransportWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(
        &out,
        &PartialOutput {
            format: "marklab.partial_ot",
            version: 1,
            source_sha256: marklab_bayes::sha256_hex(&source_bytes),
            target_sha256: marklab_bayes::sha256_hex(&target_bytes),
            cost_sha256: marklab_bayes::sha256_hex(&cost_bytes),
            backend: result.backend,
            request_sha256: result.request_sha256,
            optimizer: result.optimizer,
            plan: result.plan,
            source_marginals: result.source_marginals,
            target_marginals: result.target_marginals,
            transported_mass: result.transported_mass,
            unmatched_source_mass: result.unmatched_source_mass,
            unmatched_target_mass: result.unmatched_target_mass,
            transport_cost: result.transport_cost,
            entropy: result.entropy,
            regularized_objective: result.regularized_objective,
            maximum_constraint_violation: result.maximum_constraint_violation,
            constraint_status: result.constraint_status,
        },
    )
}

fn read_costs(
    bytes: &[u8],
    source: &[TransportMass],
    target: &[TransportMass],
) -> Result<Vec<f64>, BayesCliError> {
    let mut costs = BTreeMap::new();
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    if !reader
        .headers()?
        .iter()
        .eq(["source_id", "target_id", "cost"])
    {
        return Err(BayesCliError::Input("transport cost header differs".into()));
    }
    for row in reader.deserialize::<CostRow>() {
        let row = row?;
        if costs
            .insert((row.source_id, row.target_id), row.cost)
            .is_some()
        {
            return Err(BayesCliError::Input("duplicate transport cost pair".into()));
        }
    }
    let matrix = source
        .iter()
        .flat_map(|source_row| {
            target.iter().map(|target_row| {
                costs
                    .get(&(source_row.id.clone(), target_row.id.clone()))
                    .copied()
                    .ok_or_else(|| {
                        BayesCliError::Input("transport cost matrix is incomplete".into())
                    })
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if costs.len() != matrix.len() {
        return Err(BayesCliError::Input(
            "transport cost matrix contains undeclared identities".into(),
        ));
    }
    Ok(matrix)
}

fn read_source(bytes: &[u8]) -> Result<Vec<TransportMass>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    if !reader.headers()?.iter().eq(["source_id", "mass"]) {
        return Err(BayesCliError::Input(
            "transport source header differs".into(),
        ));
    }
    reader
        .deserialize::<SourceRow>()
        .map(|row| {
            let row = row?;
            Ok(TransportMass {
                id: row.source_id,
                mass: row.mass,
            })
        })
        .collect::<Result<Vec<_>, csv::Error>>()
        .map_err(Into::into)
}

fn read_target(bytes: &[u8]) -> Result<Vec<TransportMass>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    if !reader.headers()?.iter().eq(["target_id", "mass"]) {
        return Err(BayesCliError::Input(
            "transport target header differs".into(),
        ));
    }
    reader
        .deserialize::<TargetRow>()
        .map(|row| {
            let row = row?;
            Ok(TransportMass {
                id: row.target_id,
                mass: row.mass,
            })
        })
        .collect::<Result<Vec<_>, csv::Error>>()
        .map_err(Into::into)
}

fn map(error: TransportError) -> BayesCliError {
    BayesCliError::Input(error.to_string())
}

#[derive(Serialize)]
struct Output {
    source_sha256: String,
    target_sha256: String,
    cost_sha256: String,
    #[serde(flatten)]
    result: marklab_bayes::SinkhornResult,
}

#[derive(Serialize)]
struct UnbalancedOutput {
    source_sha256: String,
    target_sha256: String,
    cost_sha256: String,
    #[serde(flatten)]
    result: marklab_bayes::UnbalancedSinkhornResult,
}

#[derive(Serialize)]
struct SoftAssignmentOutput {
    source_sha256: String,
    target_sha256: String,
    cost_sha256: String,
    #[serde(flatten)]
    result: marklab_bayes::EntropicSoftAssignmentResult,
}

#[derive(Serialize)]
struct PartialOutput {
    format: &'static str,
    version: u32,
    backend: marklab_bayes::WorkerBackend,
    source_sha256: String,
    target_sha256: String,
    cost_sha256: String,
    request_sha256: String,
    optimizer: marklab_bayes::PartialTransportOptimizer,
    plan: Vec<marklab_bayes::PartialTransportPlanEntry>,
    source_marginals: Vec<f64>,
    target_marginals: Vec<f64>,
    transported_mass: f64,
    unmatched_source_mass: Vec<f64>,
    unmatched_target_mass: Vec<f64>,
    transport_cost: f64,
    entropy: f64,
    regularized_objective: f64,
    maximum_constraint_violation: f64,
    constraint_status: String,
}

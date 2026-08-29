use std::path::PathBuf;

use crate::{
    execute_algorithm_with_store,
    inhomogeneous_spatial::encode_piecewise_compartment_pair_correlation_result, ArtifactSchema,
    LocalScheduler, MarklabError, NodeId, PiecewiseCompartmentPairCorrelationAnalysisNode,
    PiecewiseCompartmentPairCorrelationConfig, PiecewiseCompartmentSpatialConfig,
    PiecewiseCompartmentSpatialLimits, Result, SchedulerLimits, WorkflowGraph,
};

use super::{
    classical::native_runtime_provenance,
    piecewise_compartment_spatial::{
        prepare, write_output, PrepareRequest, PreparedPiecewiseCompartmentProject,
    },
};

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub observation_mask: PathBuf,
    pub negative_mask: PathBuf,
    pub positive_mask: PathBuf,
    pub negative_compartment_id: String,
    pub positive_compartment_id: String,
    pub out: PathBuf,
    pub radii_um: Vec<f64>,
    pub pair_bandwidth_um: f64,
    pub simulations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub memory_budget_mib: usize,
    pub maximum_boundary_segments: usize,
    pub maximum_compartment_queries: usize,
    pub maximum_pair_visits: usize,
    pub maximum_null_draws: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let PreparedPiecewiseCompartmentProject {
        mut durable,
        mut project,
        store,
        pattern,
        partition,
        memory_bytes,
    } = prepare(PrepareRequest {
        project: &request.project,
        cells: &request.cells,
        observation_mask: &request.observation_mask,
        negative_mask: &request.negative_mask,
        positive_mask: &request.positive_mask,
        negative_compartment_id: &request.negative_compartment_id,
        positive_compartment_id: &request.positive_compartment_id,
        memory_budget_mib: request.memory_budget_mib,
        maximum_boundary_segments: request.maximum_boundary_segments,
        store_id: "piecewise-compartment-pair-correlation-store",
    })?;
    let limits = PiecewiseCompartmentSpatialLimits::new(
        pattern.len(),
        request.radii_um.len(),
        request.maximum_compartment_queries,
        request.maximum_pair_visits,
        request.maximum_null_draws,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let base = PiecewiseCompartmentSpatialConfig::new(
        request.radii_um,
        request.simulations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = PiecewiseCompartmentPairCorrelationConfig::new(base, request.pair_bandwidth_um)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = PiecewiseCompartmentPairCorrelationAnalysisNode::new(
        &mut project,
        NodeId::new("piecewise-compartment-pair-correlation")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &pattern,
        &partition,
        &config,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: memory_bytes,
    })
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.piecewise_compartment_pair_correlation", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = encode_piecewise_compartment_pair_correlation_result(
        &run.output,
        &pattern,
        &partition,
        &config,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "piecewise-compartment-pair-correlation",
    )
}

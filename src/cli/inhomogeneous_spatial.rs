use std::path::PathBuf;

use crate::{
    execute_algorithm_with_store, inhomogeneous_spatial::workflow::encode_result, ArtifactSchema,
    InhomogeneousSpatialAnalysisNode, InhomogeneousSpatialConfig, InhomogeneousSpatialLimits,
    LocalScheduler, MarklabError, NodeId, Result, SchedulerLimits, WorkflowGraph,
};

use super::{
    classical::native_runtime_provenance,
    inhomogeneous_project::{prepare, write_output, PrepareRequest, PreparedInhomogeneousProject},
};

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub mask: PathBuf,
    pub out: PathBuf,
    pub radii_um: Vec<f64>,
    pub bandwidth_um: f64,
    pub integration_grid: [usize; 2],
    pub simulations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub minimum_intensity_per_um2: f64,
    pub memory_budget_mib: usize,
    pub maximum_probes: usize,
    pub maximum_intensity_evaluations: usize,
    pub maximum_pair_visits: usize,
    pub maximum_null_draws: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let PreparedInhomogeneousProject {
        mut durable,
        mut project,
        store,
        pattern,
        window,
        memory_bytes,
    } = prepare(PrepareRequest {
        project: &request.project,
        cells: &request.cells,
        mask: &request.mask,
        memory_budget_mib: request.memory_budget_mib,
        store_id: "inhomogeneous-spatial-store",
        source_change_message:
            "inhomogeneous spatial source changed while its durable input was prepared",
    })?;
    let limits = InhomogeneousSpatialLimits::new(
        pattern.len(),
        request.radii_um.len(),
        request.maximum_probes,
        request.maximum_intensity_evaluations,
        request.maximum_pair_visits,
        request.maximum_null_draws,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = InhomogeneousSpatialConfig::new(
        request.radii_um,
        request.bandwidth_um,
        request.integration_grid,
        request.simulations,
        request.seed,
        request.alpha,
        request.minimum_intensity_per_um2,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = InhomogeneousSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("inhomogeneous-spatial")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &pattern,
        &window,
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
        ArtifactSchema::new("marklab.inhomogeneous_spatial", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = encode_result(&run.output, &pattern, &window, &config)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "inhomogeneous-spatial",
    )
}

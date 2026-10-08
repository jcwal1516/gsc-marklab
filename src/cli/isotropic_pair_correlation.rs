use super::{
    classical::{native_runtime_provenance, source_artifact},
    inhomogeneous_project::{prepare, write_output, PrepareRequest, PreparedInhomogeneousProject},
};
use crate::{
    execute_algorithm_with_store, ArtifactSchema, IsotropicPairCorrelationAnalysisNode,
    IsotropicPairCorrelationConfig, IsotropicPairCorrelationResultDocument, IsotropicSpatialLimits,
    LocalScheduler, MarklabError, NodeId, Result, SchedulerLimits, WorkflowGraph,
};
use std::path::PathBuf;

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub mask: PathBuf,
    pub out: PathBuf,
    pub radii_um: Vec<f64>,
    pub bandwidth_um: f64,
    pub simulations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub memory_budget_mib: usize,
    pub maximum_pair_visits: usize,
    pub maximum_visible_arc_evaluations: usize,
    pub maximum_arc_segment_tests: usize,
    pub maximum_arc_membership_queries: usize,
    pub maximum_csr_draws: usize,
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
        store_id: "isotropic-pair-correlation-store",
        source_change_message:
            "isotropic pair-correlation source changed while its durable input was prepared",
    })?;
    let limits = IsotropicSpatialLimits::new(
        pattern.len().max(1),
        request.radii_um.len().max(1),
        request.maximum_pair_visits,
        request.maximum_visible_arc_evaluations,
        request.maximum_arc_segment_tests,
        request.maximum_arc_membership_queries,
        request.maximum_csr_draws,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = IsotropicPairCorrelationConfig::new(
        request.radii_um,
        request.bandwidth_um,
        request.simulations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let runtime = native_runtime_provenance()?;
    let identity = format!(
        "{};adapter=isotropic-pair-correlation-node-v1",
        runtime.implementation_identity()
    );
    let sources = vec![
        source_artifact(
            &request.cells,
            "application/vnd.marklab.source.point-table;version=1",
        )?,
        source_artifact(
            &request.mask,
            "application/vnd.marklab.source.observation-window;version=1",
        )?,
    ];
    let node = IsotropicPairCorrelationAnalysisNode::new_with_implementation_identity(
        &mut project,
        NodeId::new("isotropic-pair-correlation")
            .map_err(|e| MarklabError::Validation(e.to_string()))?,
        &pattern,
        &window,
        &config,
        identity,
        sources,
    )
    .map_err(|e| MarklabError::Validation(e.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|e| MarklabError::Validation(e.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: memory_bytes,
    })
    .map_err(|e| MarklabError::Validation(e.to_string()))?;
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.isotropic_pair_correlation", 1)
            .map_err(|e| MarklabError::Validation(e.to_string()))?,
        runtime,
        &store,
    )
    .map_err(|e| MarklabError::Compute(e.to_string()))?;
    let encoded = IsotropicPairCorrelationResultDocument::new(run.output)
        .and_then(|document| document.to_json_pretty())?
        .into_bytes();
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "isotropic-pair-correlation",
    )
}

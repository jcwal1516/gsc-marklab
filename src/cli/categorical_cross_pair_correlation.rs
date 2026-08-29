use std::path::PathBuf;

use crate::{
    cross_pair_correlation, execute_algorithm_with_store, ArtifactSchema,
    CategoricalCrossPairCorrelationAnalysisNode, CategoricalCrossPairCorrelationConfig,
    CategoricalPairLimits, DeclaredScalarPatternInput, LocalScheduler, MarklabError, NodeId,
    Result, SchedulerLimits, WorkflowGraph,
};

use super::{
    categorical_mark_project::{prepare, write_output, PrepareRequest},
    classical::native_runtime_provenance,
};

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub mask: PathBuf,
    pub out: PathBuf,
    pub source_level: String,
    pub target_level: String,
    pub radii_um: Vec<f64>,
    pub bandwidth_um: f64,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub memory_budget_mib: usize,
    pub maximum_pair_visits: usize,
    pub maximum_null_pair_evaluations: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let mut prepared = prepare(PrepareRequest {
        project: &request.project,
        cells: &request.cells,
        mask: &request.mask,
        memory_budget_mib: request.memory_budget_mib,
        store_id: "categorical-cross-g-store",
    })?;
    let input = DeclaredScalarPatternInput::from_mark_table(
        &prepared.project,
        &prepared.pattern,
        &prepared.table,
        prepared.slide_id.clone(),
        prepared.frame_id.clone(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let limits = CategoricalPairLimits::new(
        prepared.pattern.len(),
        request.radii_um.len(),
        request.maximum_pair_visits,
        request.maximum_null_pair_evaluations,
        prepared.memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = CategoricalCrossPairCorrelationConfig::new(
        request.radii_um,
        request.bandwidth_um,
        request.source_level,
        request.target_level,
        request.permutations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = CategoricalCrossPairCorrelationAnalysisNode::new(
        &mut prepared.project,
        NodeId::new("categorical-cross-pair-correlation")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &input,
        &prepared.window,
        &config,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: prepared.memory_bytes,
    })
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let run = execute_algorithm_with_store(
        &mut prepared.durable,
        &mut prepared.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.categorical_cross_pair_correlation", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &prepared.store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded =
        cross_pair_correlation::encode_result(&run.output, &input, &prepared.window, &config)
            .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "categorical-cross-pair-correlation",
    )
}

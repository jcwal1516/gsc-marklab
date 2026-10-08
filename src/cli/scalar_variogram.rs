use std::path::PathBuf;

use crate::{
    execute_algorithm_with_store, ArtifactSchema, DeclaredScalarPatternInput, LocalScheduler,
    MarklabError, NodeId, Result, ScalarMarkId, ScalarVariogramAnalysisNode, ScalarVariogramBin,
    ScalarVariogramInferenceDesign, ScalarVariogramInferenceLimits, SchedulerLimits, WorkflowGraph,
    WorkflowNode,
};

use super::{
    categorical_mark_project::{prepare_nucleus_area, write_output, PrepareRequest},
    classical::native_runtime_provenance,
};

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub mask: PathBuf,
    pub out: PathBuf,
    pub lag_edges_um: Vec<f64>,
    pub condition_by_histologic_compartment: bool,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub memory_budget_mib: usize,
    pub maximum_pair_visits: usize,
    pub maximum_permutation_pair_evaluations: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let bins = bins(&request.lag_edges_um)?;
    let mut prepared = prepare_nucleus_area(PrepareRequest {
        project: &request.project,
        cells: &request.cells,
        mask: &request.mask,
        memory_budget_mib: request.memory_budget_mib,
        store_id: "scalar-variogram-store",
    })?;
    let input = DeclaredScalarPatternInput::from_mark_table(
        &prepared.project,
        &prepared.pattern,
        &prepared.table,
        prepared.slide_id.clone(),
        prepared.frame_id.clone(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let design = if request.condition_by_histologic_compartment {
        ScalarVariogramInferenceDesign::histologic_compartment_random_labeling(
            request.permutations,
            request.seed,
            request.alpha,
        )
    } else {
        ScalarVariogramInferenceDesign::random_labeling(
            request.permutations,
            request.seed,
            request.alpha,
        )
    }
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let limits = ScalarVariogramInferenceLimits::new(
        prepared.pattern.len(),
        request.maximum_pair_visits,
        request.maximum_permutation_pair_evaluations,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let mark_id = ScalarMarkId::new("nucleus_area_um2")
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = ScalarVariogramAnalysisNode::new(
        &mut prepared.project,
        NodeId::new("scalar-variogram")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &input,
        &prepared.window,
        &mark_id,
        &bins,
        &design,
        limits,
        prepared.memory_bytes,
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
        ArtifactSchema::new("marklab.scalar_variogram_inference", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &prepared.store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = node
        .encode_output(&run.output)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(&request.out, &encoded, run.cache_status, "scalar-variogram")
}

fn bins(edges: &[f64]) -> Result<Vec<ScalarVariogramBin>> {
    if edges.len() < 2 {
        return Err(MarklabError::Validation(
            "--lag-edges-um requires at least two edges".into(),
        ));
    }
    edges
        .windows(2)
        .map(|edge| {
            ScalarVariogramBin::new(edge[0], edge[1])
                .map_err(|error| MarklabError::Validation(error.to_string()))
        })
        .collect()
}

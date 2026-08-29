use std::path::PathBuf;

use crate::{
    execute_algorithm_with_store, ArtifactSchema, LocalScheduler, MarklabError, NodeId, Result,
    SchedulerLimits, TranslationSpatialAnalysisNode, TranslationSpatialConfig,
    TranslationSpatialLimits, TranslationSpatialResultDocument, WorkflowGraph,
};

use super::{
    classical::{native_runtime_provenance, source_artifact},
    inhomogeneous_project::{prepare, write_output, PrepareRequest, PreparedInhomogeneousProject},
};

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub mask: PathBuf,
    pub out: PathBuf,
    pub radii_um: Vec<f64>,
    pub simulations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub memory_budget_mib: usize,
    pub maximum_pair_visits: usize,
    pub maximum_overlap_evaluations: usize,
    pub maximum_overlap_candidate_work: usize,
    pub maximum_overlap_output_vertices: usize,
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
        store_id: "translation-spatial-store",
        source_change_message:
            "translation spatial source changed while its durable input was prepared",
    })?;
    let limits = TranslationSpatialLimits::new(
        pattern.len().max(1),
        request.radii_um.len().max(1),
        request.maximum_pair_visits,
        request.maximum_overlap_evaluations,
        request.maximum_overlap_candidate_work,
        request.maximum_overlap_output_vertices,
        request.maximum_csr_draws,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = TranslationSpatialConfig::new(
        request.radii_um,
        request.simulations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let runtime = native_runtime_provenance()?;
    let implementation_identity = format!(
        "{};adapter=translation-spatial-analysis-node-v1;geo=0.33.1",
        runtime.implementation_identity()
    );
    let source_artifacts = vec![
        source_artifact(
            &request.cells,
            "application/vnd.marklab.source.point-table;version=1",
        )?,
        source_artifact(
            &request.mask,
            "application/vnd.marklab.source.observation-window;version=1",
        )?,
    ];
    let node = TranslationSpatialAnalysisNode::new_with_implementation_identity(
        &mut project,
        NodeId::new("translation-spatial")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &pattern,
        &window,
        &config,
        implementation_identity,
        source_artifacts,
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
        ArtifactSchema::new("marklab.translation_spatial", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        runtime,
        &store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = TranslationSpatialResultDocument::new(run.output)
        .and_then(|document| document.to_json_pretty())?
        .into_bytes();
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "translation-spatial",
    )
}

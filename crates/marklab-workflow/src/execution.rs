use thiserror::Error;

use crate::{
    ArtifactSchema, CacheStatus, ContentDigest, DurableExecutionRequest, DurableProject,
    DurableProjectError, LocalArtifactStore, LocalScheduler, MarklabProject,
    NativeRuntimeProvenance, NodeRun, ProjectError, WorkflowError, WorkflowGraph, WorkflowNode,
};

/// Execute one typed algorithm through verified durable replay and commit.
///
/// The node owns input/schema validation, execution, diagnostics embedded in its typed output,
/// canonical encoding, and cache material. The scheduler remains the sole cache-key and in-memory
/// commit owner; the durable project remains the sole cross-process object/ledger/head owner.
/// Declared dependency outputs must already have been executed or restored into `project`.
pub fn execute_algorithm<N: WorkflowNode>(
    durable: &mut DurableProject,
    project: &mut MarklabProject,
    graph: &WorkflowGraph,
    node: &N,
    scheduler: &LocalScheduler,
    result_schema: ArtifactSchema,
    runtime: NativeRuntimeProvenance,
) -> Result<NodeRun<N::Output>, ExecuteAlgorithmError> {
    execute_algorithm_inner(
        durable,
        project,
        graph,
        node,
        scheduler,
        result_schema,
        runtime,
        None,
    )
}

/// Execute one typed algorithm through durable replay while verifying schema-bound inputs.
///
/// This is the store-aware form of [`execute_algorithm`]. It retains the same durable request,
/// restore, scheduler, publication, ledger, and head owners, but selects
/// [`LocalScheduler::run_single_with_store`] so every semantic input is verified in `store` before
/// a cache hit or execution is accepted.
#[allow(clippy::too_many_arguments)]
pub fn execute_algorithm_with_store<N: WorkflowNode>(
    durable: &mut DurableProject,
    project: &mut MarklabProject,
    graph: &WorkflowGraph,
    node: &N,
    scheduler: &LocalScheduler,
    result_schema: ArtifactSchema,
    runtime: NativeRuntimeProvenance,
    store: &LocalArtifactStore,
) -> Result<NodeRun<N::Output>, ExecuteAlgorithmError> {
    execute_algorithm_inner(
        durable,
        project,
        graph,
        node,
        scheduler,
        result_schema,
        runtime,
        Some(store),
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_algorithm_inner<N: WorkflowNode>(
    durable: &mut DurableProject,
    project: &mut MarklabProject,
    graph: &WorkflowGraph,
    node: &N,
    scheduler: &LocalScheduler,
    result_schema: ArtifactSchema,
    runtime: NativeRuntimeProvenance,
    store: Option<&LocalArtifactStore>,
) -> Result<NodeRun<N::Output>, ExecuteAlgorithmError> {
    let cache_key = scheduler.cache_key_for(node)?;
    let material = node.cache_key_material();
    let request = DurableExecutionRequest::new(
        node.spec().id().as_str(),
        node.spec().digest(),
        node.input_artifacts().to_vec(),
        material.configuration_digest,
        ContentDigest::from_bytes(material.execution_policy),
        scheduler.limits.max_inline_output_bytes,
        cache_key,
        result_schema,
        runtime,
    )?;
    durable.restore_success(&request, project)?;
    let run = match store {
        Some(store) => scheduler.run_single_with_store(project, graph, node, store)?,
        None => scheduler.run_single(project, graph, node)?,
    };
    if run.cache_status == CacheStatus::Miss {
        let encoded = project.read_inline_verified(&run.artifact)?.to_vec();
        durable.commit_success(&request, run.artifact.kind(), &encoded)?;
    }
    Ok(run)
}

/// Failure from the unified typed durable algorithm boundary.
#[derive(Debug, Error)]
pub enum ExecuteAlgorithmError {
    /// Graph, node, scheduler, codec, or in-memory commit failure.
    #[error(transparent)]
    Workflow(#[from] WorkflowError),
    /// Durable restore, verification, publication, ledger, head, or recovery failure.
    #[error(transparent)]
    Durable(#[from] DurableProjectError),
    /// Verified inline scheduler output could not be read for durable publication.
    #[error(transparent)]
    Project(#[from] ProjectError),
}

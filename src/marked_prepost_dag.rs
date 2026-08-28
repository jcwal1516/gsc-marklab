use std::{path::Path, thread};

use marklab_workflow::{
    execute_algorithm, ArtifactRecordError, ArtifactSchema, DurableProject, DurableProjectError,
    DurableProjectLimits, ExecuteAlgorithmError, LocalScheduler, MarklabProject,
    NativeRuntimeProvenance, NodeError, NodeId, NodeRun, ProjectError, SchedulerLimits,
    WorkflowError, WorkflowGraph, WorkflowNode,
};
use thiserror::Error;

use crate::{
    AnalysisConfig, MarkedAnalysisNode, MarkedPatternResult, MarkedPrePostNode, Pattern,
    PrePostResult, ThreadSetting,
};

const PRE_NODE_ID: &str = "marked-pre";
const POST_NODE_ID: &str = "marked-post";
const COMPARISON_NODE_ID: &str = "marked-prepost";

/// Host-wide ceilings used to plan the concrete marked pre/post project DAG.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkedPrePostDagLimits {
    maximum_parallel_nodes: usize,
    maximum_parallel_threads: usize,
    maximum_parallel_memory_mib: usize,
    maximum_total_pattern_rows: usize,
}

impl MarkedPrePostDagLimits {
    /// Validate positive node, thread, memory, and retained-row ceilings.
    pub fn new(
        maximum_parallel_nodes: usize,
        maximum_parallel_threads: usize,
        maximum_parallel_memory_mib: usize,
        maximum_total_pattern_rows: usize,
    ) -> Result<Self, MarkedPrePostDagError> {
        if [
            maximum_parallel_nodes,
            maximum_parallel_threads,
            maximum_parallel_memory_mib,
            maximum_total_pattern_rows,
        ]
        .contains(&0)
        {
            return Err(MarkedPrePostDagError::InvalidLimits);
        }
        Ok(Self {
            maximum_parallel_nodes,
            maximum_parallel_threads,
            maximum_parallel_memory_mib,
            maximum_total_pattern_rows,
        })
    }
}

/// Exact execution target for an incrementally resumable marked DAG.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkedPrePostDagTarget {
    /// Execute or restore only the two independent marked analyses.
    RootAnalyses,
    /// Execute or restore both roots and their typed pre/post comparison.
    Comparison,
}

/// Deterministic resource plan for the fixed two-root, one-comparison DAG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkedPrePostDagPlan {
    waves: Vec<Vec<NodeId>>,
    root_parallelism: usize,
    threads_per_root: usize,
    memory_mib_per_root: usize,
    total_pattern_rows: usize,
}

impl MarkedPrePostDagPlan {
    /// Number of roots executed concurrently in the first wave.
    pub fn root_parallelism(&self) -> usize {
        self.root_parallelism
    }

    /// Number of dependency-ordered waves in the complete plan.
    pub fn wave_count(&self) -> usize {
        self.waves.len()
    }

    /// Canonically ordered node identities in every planned wave.
    pub fn waves(&self) -> &[Vec<NodeId>] {
        &self.waves
    }

    /// Declared analysis threads reserved by each root node.
    pub fn threads_per_root(&self) -> usize {
        self.threads_per_root
    }

    /// Declared analysis memory reserved by each root node.
    pub fn memory_mib_per_root(&self) -> usize {
        self.memory_mib_per_root
    }

    /// Combined pre/post Pattern rows admitted by the plan.
    pub fn total_pattern_rows(&self) -> usize {
        self.total_pattern_rows
    }
}

/// Typed results and durable dispositions from one incremental DAG invocation.
pub struct MarkedPrePostDagRun {
    /// Exact resource plan used by this invocation.
    pub plan: MarkedPrePostDagPlan,
    /// Pre-analysis result or verified durable replay.
    pub pre: NodeRun<MarkedPatternResult>,
    /// Post-analysis result or verified durable replay.
    pub post: NodeRun<MarkedPatternResult>,
    /// Comparison result when [`MarkedPrePostDagTarget::Comparison`] was requested.
    pub comparison: Option<NodeRun<PrePostResult>>,
    /// Executed or restored plan-wave count for this target.
    pub executed_wave_count: usize,
    /// Durable ledger rows for pre, post, and comparison node shards.
    pub durable_execution_counts: [usize; 3],
}

/// Plan the smallest current dependency-bearing marked workflow without executing it.
pub fn plan_marked_prepost_dag(
    pre: &Pattern,
    post: &Pattern,
    config: &AnalysisConfig,
    limits: MarkedPrePostDagLimits,
) -> Result<MarkedPrePostDagPlan, MarkedPrePostDagError> {
    let threads_per_root = match config.performance.threads {
        ThreadSetting::Count(threads) if threads > 0 => threads,
        _ => return Err(MarkedPrePostDagError::UnboundedThreadSetting),
    };
    let memory_mib_per_root = config.performance.memory_budget_mib;
    if memory_mib_per_root == 0
        || threads_per_root > limits.maximum_parallel_threads
        || memory_mib_per_root > limits.maximum_parallel_memory_mib
    {
        return Err(MarkedPrePostDagError::RootResourceLimitExceeded);
    }
    let total_pattern_rows = pre
        .len()
        .checked_add(post.len())
        .ok_or(MarkedPrePostDagError::SizeOverflow)?;
    if total_pattern_rows > limits.maximum_total_pattern_rows {
        return Err(MarkedPrePostDagError::PatternRowLimitExceeded {
            observed: total_pattern_rows,
            maximum: limits.maximum_total_pattern_rows,
        });
    }
    let parallel_threads = threads_per_root
        .checked_mul(2)
        .ok_or(MarkedPrePostDagError::SizeOverflow)?;
    let parallel_memory = memory_mib_per_root
        .checked_mul(2)
        .ok_or(MarkedPrePostDagError::SizeOverflow)?;
    let root_parallelism = if limits.maximum_parallel_nodes >= 2
        && parallel_threads <= limits.maximum_parallel_threads
        && parallel_memory <= limits.maximum_parallel_memory_mib
    {
        2
    } else {
        1
    };
    let pre_id = NodeId::new(PRE_NODE_ID)?;
    let post_id = NodeId::new(POST_NODE_ID)?;
    let comparison_id = NodeId::new(COMPARISON_NODE_ID)?;
    let waves = if root_parallelism == 2 {
        vec![vec![pre_id, post_id], vec![comparison_id]]
    } else {
        vec![vec![pre_id], vec![post_id], vec![comparison_id]]
    };
    Ok(MarkedPrePostDagPlan {
        waves,
        root_parallelism,
        threads_per_root,
        memory_mib_per_root,
        total_pattern_rows,
    })
}

/// Execute or resume the fixed marked pre/post DAG through bounded durable node shards.
///
/// Independent roots run concurrently only when the declared node/thread/memory plan admits both.
/// Every node shard delegates cache keys, objects, pending-intent recovery, ledger entries, and head
/// updates to [`DurableProject`]. Root outputs are then merged into one in-memory project solely so
/// the existing scheduler can verify both exact dependency edges before comparison.
#[allow(clippy::too_many_arguments)]
pub fn execute_marked_prepost_dag(
    root: &Path,
    pre: &Pattern,
    post: &Pattern,
    config: &AnalysisConfig,
    target: MarkedPrePostDagTarget,
    limits: MarkedPrePostDagLimits,
    durable_limits: DurableProjectLimits,
    scheduler_limits: SchedulerLimits,
    runtime: NativeRuntimeProvenance,
) -> Result<MarkedPrePostDagRun, MarkedPrePostDagError> {
    let plan = plan_marked_prepost_dag(pre, post, config, limits)?;
    let scheduler = LocalScheduler::new(scheduler_limits)?;
    let mut pre_project = MarklabProject::new();
    let mut post_project = MarklabProject::new();
    let pre_node =
        MarkedAnalysisNode::new(&mut pre_project, NodeId::new(PRE_NODE_ID)?, pre, config)?;
    let post_node =
        MarkedAnalysisNode::new(&mut post_project, NodeId::new(POST_NODE_ID)?, post, config)?;
    let root_graph = WorkflowGraph::new([pre_node.spec().clone(), post_node.spec().clone()])?;
    let marked_schema = ArtifactSchema::new("marklab.marked_pattern_result", 3)?;
    let mut pre_durable = DurableProject::open_or_create(&root.join(PRE_NODE_ID), durable_limits)?;
    let mut post_durable =
        DurableProject::open_or_create(&root.join(POST_NODE_ID), durable_limits)?;

    let (pre_run, post_run) = if plan.root_parallelism == 2 {
        thread::scope(|scope| {
            let pre_runtime = runtime.clone();
            let pre_schema = marked_schema.clone();
            let pre_handle = scope.spawn(|| {
                execute_algorithm(
                    &mut pre_durable,
                    &mut pre_project,
                    &root_graph,
                    &pre_node,
                    &scheduler,
                    pre_schema,
                    pre_runtime,
                )
            });
            let post_handle = scope.spawn(|| {
                execute_algorithm(
                    &mut post_durable,
                    &mut post_project,
                    &root_graph,
                    &post_node,
                    &scheduler,
                    marked_schema,
                    runtime.clone(),
                )
            });
            let pre_result = pre_handle
                .join()
                .map_err(|_| MarkedPrePostDagError::WorkerPanicked { node: PRE_NODE_ID })?;
            let post_result = post_handle
                .join()
                .map_err(|_| MarkedPrePostDagError::WorkerPanicked { node: POST_NODE_ID })?;
            Ok::<_, MarkedPrePostDagError>((pre_result?, post_result?))
        })?
    } else {
        let pre_run = execute_algorithm(
            &mut pre_durable,
            &mut pre_project,
            &root_graph,
            &pre_node,
            &scheduler,
            marked_schema.clone(),
            runtime.clone(),
        )?;
        let post_run = execute_algorithm(
            &mut post_durable,
            &mut post_project,
            &root_graph,
            &post_node,
            &scheduler,
            marked_schema,
            runtime.clone(),
        )?;
        (pre_run, post_run)
    };
    let root_wave_count = if plan.root_parallelism == 2 { 1 } else { 2 };
    let mut durable_execution_counts = [
        pre_durable.execution_count(),
        post_durable.execution_count(),
        0,
    ];
    if target == MarkedPrePostDagTarget::RootAnalyses {
        return Ok(MarkedPrePostDagRun {
            plan,
            pre: pre_run,
            post: post_run,
            comparison: None,
            executed_wave_count: root_wave_count,
            durable_execution_counts,
        });
    }

    merge_root_run(&mut pre_project, &post_node, &post_run)?;
    let comparison_node = MarkedPrePostNode::new(
        NodeId::new(COMPARISON_NODE_ID)?,
        pre_node.spec().id().clone(),
        &pre_run,
        post_node.spec().id().clone(),
        &post_run,
    )?;
    let graph = WorkflowGraph::new([
        pre_node.spec().clone(),
        post_node.spec().clone(),
        comparison_node.spec().clone(),
    ])?;
    let mut comparison_durable =
        DurableProject::open_or_create(&root.join(COMPARISON_NODE_ID), durable_limits)?;
    let comparison = execute_algorithm(
        &mut comparison_durable,
        &mut pre_project,
        &graph,
        &comparison_node,
        &scheduler,
        ArtifactSchema::new("marklab.marked_prepost_result", 3)?,
        runtime,
    )?;
    durable_execution_counts[2] = comparison_durable.execution_count();
    Ok(MarkedPrePostDagRun {
        plan,
        pre: pre_run,
        post: post_run,
        comparison: Some(comparison),
        executed_wave_count: root_wave_count + 1,
        durable_execution_counts,
    })
}

fn merge_root_run(
    project: &mut MarklabProject,
    node: &MarkedAnalysisNode<'_>,
    run: &NodeRun<MarkedPatternResult>,
) -> Result<(), MarkedPrePostDagError> {
    let encoded = node.encode_output(&run.output)?;
    run.artifact.verify_bytes(&encoded)?;
    let merged = project.commit_workflow_success(
        node.spec().id().as_str(),
        node.spec().digest(),
        run.cache_key,
        node.output_kind(),
        encoded,
    )?;
    if merged != run.artifact {
        return Err(MarkedPrePostDagError::RootMergeMismatch);
    }
    Ok(())
}

/// Failure while planning, executing, restoring, or composing the marked DAG.
#[derive(Debug, Error)]
pub enum MarkedPrePostDagError {
    /// One or more DAG-level resource ceilings are zero.
    #[error("marked pre/post DAG resource limits must be positive")]
    InvalidLimits,
    /// Automatic or zero thread selection cannot form a host-bounded resource plan.
    #[error("marked pre/post DAG requires an explicit positive thread count")]
    UnboundedThreadSetting,
    /// One root exceeds the available thread or memory ceiling by itself.
    #[error("one marked DAG root exceeds the declared host thread or memory ceiling")]
    RootResourceLimitExceeded,
    /// Combined input rows exceed the caller's retained-row ceiling.
    #[error("marked DAG has {observed} total Pattern rows; maximum is {maximum}")]
    PatternRowLimitExceeded {
        /// Combined pre/post rows.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Checked resource arithmetic overflowed.
    #[error("marked pre/post DAG resource arithmetic overflow")]
    SizeOverflow,
    /// One bounded root worker panicked before returning an explicit error.
    #[error("marked pre/post DAG worker {node} panicked")]
    WorkerPanicked {
        /// Fixed root-node identity.
        node: &'static str,
    },
    /// The reconstructed dependency artifact differs from the durable root result.
    #[error("marked pre/post DAG root merge changed the exact output artifact")]
    RootMergeMismatch,
    /// Node construction or output re-encoding failed.
    #[error(transparent)]
    Node(#[from] NodeError),
    /// Graph or scheduler construction failed.
    #[error(transparent)]
    Workflow(#[from] WorkflowError),
    /// Unified node execution or replay failed.
    #[error(transparent)]
    Execute(#[from] ExecuteAlgorithmError),
    /// Durable project open or recovery failed before node execution.
    #[error(transparent)]
    Durable(#[from] DurableProjectError),
    /// In-memory dependency reconstruction failed.
    #[error(transparent)]
    Project(#[from] ProjectError),
    /// Result-schema construction or artifact identity verification failed.
    #[error(transparent)]
    Artifact(#[from] ArtifactRecordError),
}

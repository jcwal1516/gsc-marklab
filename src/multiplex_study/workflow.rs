use std::path::Path;

use marklab_workflow::{
    execute_algorithm, ArtifactSchema, CacheStatus, DurableProject, DurableProjectLimits,
    LocalScheduler, MarklabProject, NativeRuntimeProvenance, SchedulerLimits, WorkflowGraph,
    WorkflowNode,
};

use super::{
    admission::PreparedStudy,
    invalid,
    model::{MultiplexStudyResult, SlideResult},
    nodes::{CohortNode, SlideBatchNode, SlideNode},
};
use crate::Result;

const MAXIMUM_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

/// Complete the study or stop at a reproducible boundary in its canonical slide order.
#[derive(Clone, Copy, Debug)]
pub enum MultiplexStudyTarget {
    /// Restore/execute all slides and patient inference.
    Complete,
    /// Restore/execute the first positive number of slides, then return without a final report.
    ThroughSlides(usize),
}

/// Scientific output and honest execution dispositions from a resumable study invocation.
pub struct MultiplexStudyRun {
    /// Present only after every slide and the patient node completes (including diagnostic output).
    pub result: Option<MultiplexStudyResult>,
    /// Slide statistics actually executed during this invocation.
    pub executed_slides: usize,
    /// Slide statistics restored from verified durable artifacts.
    pub restored_slides: usize,
    /// Append-only successful node executions, including any collection and patient nodes.
    pub durable_execution_count: usize,
}

/// Execute a recipe through the existing durable scheduler and exact-float codecs.
///
/// The root is one local project; no new worktree or external runtime is created. Sources and
/// design are admitted before project mutation. Work is serial. Up to 1,024 slides are reduced
/// through concrete batches of at most 64 dependencies, preserving the durable input bound.
/// Resume verifies all inputs and dependency outputs; completed statistics and Max-T are restored,
/// not refitted. Publication is a separate operation so partial runs never masquerade as reports.
pub fn execute_multiplex_study(
    root: &Path,
    recipe_json: &[u8],
    target: MultiplexStudyTarget,
    runtime: NativeRuntimeProvenance,
) -> Result<MultiplexStudyRun> {
    let study = PreparedStudy::from_json(recipe_json)?;
    let stop = match target {
        MultiplexStudyTarget::Complete => study.slides.len(),
        MultiplexStudyTarget::ThroughSlides(count) if count > 0 && count <= study.slides.len() => {
            count
        }
        _ => {
            return Err(invalid(
                "slide checkpoint must be within the admitted slide count",
            ))
        }
    };
    let limits = DurableProjectLimits::new(
        1024 * 1024,
        64 * 1024 * 1024,
        100_000,
        1024 * 1024,
        MAXIMUM_OUTPUT_BYTES,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let mut durable =
        DurableProject::open_or_create(root, limits).map_err(|error| invalid(error.to_string()))?;
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_OUTPUT_BYTES)
        .map_err(|error| invalid(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_OUTPUT_BYTES,
    })
    .map_err(|error| invalid(error.to_string()))?;
    let mut nodes = study
        .slides
        .iter()
        .map(|slide| SlideNode::new(&study, slide))
        .collect::<Result<Vec<_>>>()?;
    let mut specs = nodes
        .iter()
        .map(|node| node.spec().clone())
        .collect::<Vec<_>>();
    let graph = WorkflowGraph::new(specs.clone()).map_err(|error| invalid(error.to_string()))?;
    let mut slides = Vec::with_capacity(stop);
    let mut inputs = Vec::with_capacity(stop);
    let mut executed_slides = 0;
    let mut restored_slides = 0;
    let mut total_edge_work = 0usize;
    for node in nodes.iter_mut().take(stop) {
        node.remaining_edge_work = study.recipe.limits.maximum_edge_evaluations - total_edge_work;
        for artifact in node.input_artifacts() {
            project
                .register_reference(artifact.clone())
                .map_err(|error| invalid(error.to_string()))?;
        }
        let run = execute_algorithm(
            &mut durable,
            &mut project,
            &graph,
            node,
            &scheduler,
            schema("marklab.multiplex_slide")?,
            runtime.clone(),
        )
        .map_err(|error| invalid(error.to_string()))?;
        count_edge_work(
            &mut total_edge_work,
            &run.output,
            study.recipe.limits.maximum_edge_evaluations,
        )?;
        match run.cache_status {
            CacheStatus::Miss => executed_slides += 1,
            CacheStatus::Hit => restored_slides += 1,
        }
        inputs.push(run.artifact);
        slides.push(run.output);
    }
    if matches!(target, MultiplexStudyTarget::ThroughSlides(_)) {
        return Ok(MultiplexStudyRun {
            result: None,
            executed_slides,
            restored_slides,
            durable_execution_count: durable.execution_count(),
        });
    }
    let mut dependencies = specs
        .iter()
        .map(|spec| spec.id().clone())
        .collect::<Vec<_>>();
    if slides.len() > 64 {
        let mut batch_inputs = Vec::new();
        let mut batch_dependencies = Vec::new();
        for (index, rows) in slides.chunks(64).enumerate() {
            let start = index * 64;
            let end = start + rows.len();
            let node = SlideBatchNode::new(
                index,
                rows,
                inputs[start..end].to_vec(),
                dependencies[start..end].to_vec(),
            )?;
            specs.push(node.spec().clone());
            let batch_graph =
                WorkflowGraph::new(specs.clone()).map_err(|error| invalid(error.to_string()))?;
            let run = execute_algorithm(
                &mut durable,
                &mut project,
                &batch_graph,
                &node,
                &scheduler,
                schema("marklab.multiplex_slide_collection")?,
                runtime.clone(),
            )
            .map_err(|error| invalid(error.to_string()))?;
            batch_dependencies.push(node.spec().id().clone());
            batch_inputs.push(run.artifact);
        }
        dependencies = batch_dependencies;
        inputs = batch_inputs;
    }
    let node = CohortNode::new(&study, &slides, inputs, dependencies)?;
    specs.push(node.spec().clone());
    let graph = WorkflowGraph::new(specs).map_err(|error| invalid(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema("marklab.multiplex_study")?,
        runtime,
    )
    .map_err(|error| invalid(error.to_string()))?;
    Ok(MultiplexStudyRun {
        result: Some(run.output),
        executed_slides,
        restored_slides,
        durable_execution_count: durable.execution_count(),
    })
}

fn schema(name: &str) -> Result<ArtifactSchema> {
    ArtifactSchema::new(name, 1).map_err(|error| invalid(error.to_string()))
}

pub(super) fn count_edge_work(
    total: &mut usize,
    slide: &SlideResult,
    maximum: usize,
) -> Result<()> {
    for channel in &slide.channels {
        *total = channel
            .directed_edges
            .checked_mul(2)
            .and_then(|work| total.checked_add(work))
            .ok_or_else(|| invalid("study edge work overflow"))?;
    }
    if *total > maximum {
        return Err(invalid(format!(
            "complete study requires {total} edge evaluations; maximum is {maximum}"
        )));
    }
    Ok(())
}

use std::path::PathBuf;

use crate::{
    geom::{mask::TumorMask, window::ObservationWindowLimits},
    output::{ClassicalOutputContext, ClassicalOutputWriter},
    spatial::{ClassicalSpatialAnalysisNode, ClassicalSpatialConfig, ClassicalSpatialLimits},
    workflow::{
        execute_algorithm, ArtifactRef, ArtifactSchema, DurableProject, DurableProjectLimits,
        LocalScheduler, MarklabProject, NodeId, SchedulerLimits, WorkflowGraph,
    },
    MarklabError, ObservationWindow2D, Pattern, PatternLoader, Result,
};

use super::{
    project_runtime::{
        native_runtime_provenance, read_bounded_utf8, report_recovery, source_artifact,
    },
    ClassicalRequest,
};

const MAXIMUM_RADII: usize = 4_096;
const CELL_SOURCE_KIND: &str = "application/vnd.marklab.source.cell-table;version=1";
const WINDOW_SOURCE_KIND: &str = "application/vnd.marklab.source.observation-window;version=1";

pub(super) fn run(request: ClassicalRequest) -> Result<()> {
    let prepared = prepare(&request)?;
    let mut project = MarklabProject::with_inline_artifact_limit(prepared.memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = ClassicalSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("classical-spatial-analysis")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &prepared.pattern,
        &prepared.window,
        &prepared.config,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let scheduler = scheduler(prepared.memory_bytes)?;
    let run = scheduler
        .run_single(&mut project, &graph, &node)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(&request, "classical", run)
}

pub(super) fn run_project(project_path: PathBuf, request: ClassicalRequest) -> Result<()> {
    let (prepared, source_artifacts) = prepare_project(&request)?;
    let runtime = native_runtime_provenance()?;
    let project_limits = DurableProjectLimits::for_object_bytes(prepared.memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, project_limits)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    report_recovery(&durable);

    let mut project = MarklabProject::with_inline_artifact_limit(prepared.memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = ClassicalSpatialAnalysisNode::new_with_implementation_identity(
        &mut project,
        NodeId::new("classical-spatial-analysis")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &prepared.pattern,
        &prepared.window,
        &prepared.config,
        runtime.implementation_identity().to_owned(),
        source_artifacts,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let scheduler = scheduler(prepared.memory_bytes)?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.classical_spatial", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        runtime,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(&request, "project classical", run)
}

fn prepare_project(request: &ClassicalRequest) -> Result<(PreparedClassical, Vec<ArtifactRef>)> {
    let before = source_artifacts(request)?;
    let prepared = prepare(request)?;
    let after = source_artifacts(request)?;
    if before != after {
        return Err(MarklabError::Validation(
            "classical source files changed while the durable input was prepared".to_owned(),
        ));
    }
    Ok((prepared, after))
}

fn source_artifacts(request: &ClassicalRequest) -> Result<Vec<ArtifactRef>> {
    Ok(vec![
        source_artifact(&request.cells, CELL_SOURCE_KIND)?,
        source_artifact(&request.mask, WINDOW_SOURCE_KIND)?,
    ])
}

struct PreparedClassical {
    pattern: Pattern,
    window: ObservationWindow2D,
    config: ClassicalSpatialConfig,
    memory_bytes: usize,
}

fn prepare(request: &ClassicalRequest) -> Result<PreparedClassical> {
    validate_request(request)?;
    let memory_bytes = request
        .memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| MarklabError::Validation("--memory-budget-mib is too large".into()))?;
    let window_limits = ObservationWindowLimits::default();
    let window_text = read_bounded_utf8(&request.mask, window_limits.maximum_input_bytes)?;
    let window = ObservationWindow2D::from_geojson_str(&window_text, window_limits)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let mask = TumorMask::from_geojson_str(&window_text)?;
    let pattern = PatternLoader::new(&mask).load_classical(&request.cells)?;

    let radii = (1..=request.r_steps)
        .map(|index| request.r_max_um * (index as f64 / request.r_steps as f64))
        .collect::<Vec<_>>();
    let maximum_points = (memory_bytes / (2 * std::mem::size_of::<f64>())).max(1);
    let limits = ClassicalSpatialLimits::new(
        maximum_points,
        MAXIMUM_RADII,
        request.maximum_pair_visits,
        request.maximum_csr_draws,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = ClassicalSpatialConfig::new(
        radii,
        request.simulations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;

    Ok(PreparedClassical {
        pattern,
        window,
        config,
        memory_bytes,
    })
}

fn scheduler(memory_bytes: usize) -> Result<LocalScheduler> {
    LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: memory_bytes,
    })
    .map_err(|error| MarklabError::Validation(error.to_string()))
}

fn write_output(
    request: &ClassicalRequest,
    command: &'static str,
    run: crate::workflow::NodeRun<crate::spatial::ClassicalSpatialResult>,
) -> Result<()> {
    ClassicalOutputWriter::write(
        &run.output,
        &request.out,
        ClassicalOutputContext {
            command,
            cells: request.cells.clone(),
            mask: request.mask.clone(),
            r_max_um: request.r_max_um,
            r_steps: request.r_steps,
            memory_budget_mib: request.memory_budget_mib,
            maximum_pair_visits: request.maximum_pair_visits,
            maximum_csr_draws: request.maximum_csr_draws,
            cache_key: run.cache_key,
            cache_status: run.cache_status,
            output_artifact: run.artifact,
        },
    )
}

fn validate_request(request: &ClassicalRequest) -> Result<()> {
    if !request.r_max_um.is_finite() || request.r_max_um <= 0.0 {
        return Err(MarklabError::Validation(
            "--r-max-um must be finite and positive".into(),
        ));
    }
    if request.r_steps == 0 || request.r_steps > MAXIMUM_RADII {
        return Err(MarklabError::Validation(format!(
            "--r-steps must be in 1..={MAXIMUM_RADII}"
        )));
    }
    if request.simulations == 0
        || !request.alpha.is_finite()
        || request.alpha <= 0.0
        || request.alpha >= 1.0
        || (request.simulations.saturating_add(1) as f64) * request.alpha < 1.0
    {
        return Err(MarklabError::Validation(
            "--simulations must be positive, --alpha must be in (0, 1), and (simulations + 1) * alpha must be at least one".into(),
        ));
    }
    if request.memory_budget_mib == 0
        || request.maximum_pair_visits == 0
        || request.maximum_csr_draws == 0
    {
        return Err(MarklabError::Validation(
            "classical memory, pair-visit, and CSR-draw limits must be positive".into(),
        ));
    }
    Ok(())
}

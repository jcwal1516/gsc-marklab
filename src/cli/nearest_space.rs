use std::path::PathBuf;

use crate::{
    execute_algorithm,
    geom::{mask::TumorMask, window::ObservationWindowLimits},
    output::{NearestSpaceOutputContext, NearestSpaceOutputWriter},
    ArtifactRef, ArtifactSchema, DurableProject, DurableProjectLimits, LocalScheduler,
    MarklabError, MarklabProject, NearestSpaceAnalysisNode, NearestSpaceConfig, NearestSpaceLimits,
    NodeId, ObservationWindow2D, Pattern, PatternLoader, Result, SchedulerLimits, WorkflowGraph,
};

use super::{classical, NearestSpaceRequest};

const MAXIMUM_RADII: usize = 4_096;
const PROJECT_CONTROL_BYTES: usize = 1024 * 1024;
const PROJECT_LEDGER_BYTES: usize = 16 * 1024 * 1024;
const PROJECT_LEDGER_RECORDS: usize = 10_000;
const PROJECT_RECORD_BYTES: usize = 64 * 1024;
const CELL_SOURCE_KIND: &str = "application/vnd.marklab.source.cell-table;version=1";
const WINDOW_SOURCE_KIND: &str = "application/vnd.marklab.source.observation-window;version=1";

pub(super) fn run(request: NearestSpaceRequest) -> Result<()> {
    let prepared = prepare(&request)?;
    let mut project = MarklabProject::with_inline_artifact_limit(prepared.memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = NearestSpaceAnalysisNode::new(
        &mut project,
        node_id()?,
        &prepared.pattern,
        &prepared.window,
        &prepared.config,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let run = scheduler(prepared.memory_bytes)?
        .run_single(&mut project, &graph, &node)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(&request, "nearest-space", run)
}

pub(super) fn run_project(project_path: PathBuf, request: NearestSpaceRequest) -> Result<()> {
    let (prepared, source_artifacts) = prepare_project(&request)?;
    let runtime = classical::native_runtime_provenance()?;
    let project_limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        prepared.memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, project_limits)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    classical::report_recovery(&durable);

    let mut project = MarklabProject::with_inline_artifact_limit(prepared.memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = NearestSpaceAnalysisNode::new_with_implementation_identity(
        &mut project,
        node_id()?,
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
        ArtifactSchema::new("marklab.nearest_space", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        runtime,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(&request, "project nearest-space", run)
}

struct PreparedNearestSpace {
    pattern: Pattern,
    window: ObservationWindow2D,
    config: NearestSpaceConfig,
    memory_bytes: usize,
}

fn prepare_project(
    request: &NearestSpaceRequest,
) -> Result<(PreparedNearestSpace, Vec<ArtifactRef>)> {
    let before = source_artifacts(request)?;
    let prepared = prepare(request)?;
    let after = source_artifacts(request)?;
    if before != after {
        return Err(MarklabError::Validation(
            "nearest-space source files changed while the durable input was prepared".into(),
        ));
    }
    Ok((prepared, after))
}

fn source_artifacts(request: &NearestSpaceRequest) -> Result<Vec<ArtifactRef>> {
    Ok(vec![
        classical::source_artifact(&request.cells, CELL_SOURCE_KIND)?,
        classical::source_artifact(&request.mask, WINDOW_SOURCE_KIND)?,
    ])
}

fn prepare(request: &NearestSpaceRequest) -> Result<PreparedNearestSpace> {
    validate_request(request)?;
    let memory_bytes = request
        .memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| MarklabError::Validation("--memory-budget-mib is too large".into()))?;
    let window_limits = ObservationWindowLimits::default();
    let window_text =
        classical::read_bounded_utf8(&request.mask, window_limits.maximum_input_bytes)?;
    let window = ObservationWindow2D::from_geojson_str(&window_text, window_limits)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let mask = TumorMask::from_observation_window(window.clone());
    let pattern = PatternLoader::new(&mask).load_classical(&request.cells)?;
    let radii = (1..=request.r_steps)
        .map(|index| request.r_max_um * (index as f64 / request.r_steps as f64))
        .collect::<Vec<_>>();
    let maximum_points = (memory_bytes / (2 * std::mem::size_of::<f64>())).max(1);
    let maximum_probes = request
        .probe_grid_x
        .checked_mul(request.probe_grid_y)
        .ok_or_else(|| MarklabError::Validation("probe grid is too large".into()))?;
    let limits = NearestSpaceLimits::new(
        maximum_points,
        MAXIMUM_RADII,
        maximum_probes,
        request.maximum_nearest_queries,
        request.maximum_csr_draws,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = NearestSpaceConfig::new(
        radii,
        [request.probe_grid_x, request.probe_grid_y],
        request.simulations,
        request.seed,
        request.alpha,
        request.j_denominator_epsilon,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    Ok(PreparedNearestSpace {
        pattern,
        window,
        config,
        memory_bytes,
    })
}

fn validate_request(request: &NearestSpaceRequest) -> Result<()> {
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
    if request.probe_grid_x == 0
        || request.probe_grid_y == 0
        || request.memory_budget_mib == 0
        || request.maximum_nearest_queries == 0
        || request.maximum_csr_draws == 0
    {
        return Err(MarklabError::Validation(
            "nearest-space probe, memory, query, and CSR-draw limits must be positive".into(),
        ));
    }
    Ok(())
}

fn node_id() -> Result<NodeId> {
    NodeId::new("nearest-space-analysis")
        .map_err(|error| MarklabError::Validation(error.to_string()))
}

fn scheduler(memory_bytes: usize) -> Result<LocalScheduler> {
    LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: memory_bytes,
    })
    .map_err(|error| MarklabError::Validation(error.to_string()))
}

fn write_output(
    request: &NearestSpaceRequest,
    command: &'static str,
    run: crate::NodeRun<crate::NearestSpaceResult>,
) -> Result<()> {
    NearestSpaceOutputWriter::write(
        &run.output,
        &request.out,
        NearestSpaceOutputContext {
            command,
            cells: request.cells.clone(),
            mask: request.mask.clone(),
            memory_budget_mib: request.memory_budget_mib,
            cache_key: run.cache_key,
            cache_status: run.cache_status,
            output_artifact: run.artifact,
        },
    )
}

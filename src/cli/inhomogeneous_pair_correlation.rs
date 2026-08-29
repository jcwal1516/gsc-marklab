use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use crate::{
    execute_algorithm_with_store, inhomogeneous_spatial::encode_pair_correlation_result,
    ArtifactSchema, CacheStatus, DurableProject, DurableProjectLimits,
    InhomogeneousPairCorrelationAnalysisNode, InhomogeneousPairCorrelationConfig,
    InhomogeneousSpatialConfig, InhomogeneousSpatialLimits, LocalArtifactStore, LocalScheduler,
    MarklabError, MarklabProject, NodeId, ObservationWindow2D, ObservationWindowLimits,
    PatternLoader, Result, SchedulerLimits, StoreId, TumorMask, WorkflowGraph,
};

use super::classical::{native_runtime_provenance, read_bounded_utf8, source_artifact};

const SOURCE_CELLS_KIND: &str = "application/vnd.marklab.source.point-table;version=1";
const SOURCE_WINDOW_KIND: &str = "application/vnd.marklab.source.observation-window;version=1";
const PROJECT_CONTROL_BYTES: usize = 1024 * 1024;
const PROJECT_LEDGER_BYTES: usize = 16 * 1024 * 1024;
const PROJECT_LEDGER_RECORDS: usize = 10_000;
const PROJECT_RECORD_BYTES: usize = 64 * 1024;

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub mask: PathBuf,
    pub out: PathBuf,
    pub radii_um: Vec<f64>,
    pub intensity_bandwidth_um: f64,
    pub pair_bandwidth_um: f64,
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
    let cells_before = source_artifact(&request.cells, SOURCE_CELLS_KIND)?;
    let window_before = source_artifact(&request.mask, SOURCE_WINDOW_KIND)?;
    let memory_bytes = request
        .memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| MarklabError::Validation("--memory-budget-mib is too large".into()))?;
    if memory_bytes == 0 {
        return Err(MarklabError::Validation(
            "--memory-budget-mib must be positive".into(),
        ));
    }
    let window_limits = ObservationWindowLimits::default();
    let window_text = read_bounded_utf8(&request.mask, window_limits.maximum_input_bytes)?;
    let mask = TumorMask::from_geojson_str(&window_text)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let pattern = PatternLoader::new(&mask).load(&request.cells)?;
    let window = ObservationWindow2D::from_geojson_str(&window_text, window_limits)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let cells_after = source_artifact(&request.cells, SOURCE_CELLS_KIND)?;
    let window_after = source_artifact(&request.mask, SOURCE_WINDOW_KIND)?;
    if cells_before != cells_after || window_before != window_after {
        return Err(MarklabError::Validation(
            "inhomogeneous pair-correlation source changed while its durable input was prepared"
                .into(),
        ));
    }

    let durable_limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&request.project, durable_limits)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let store_path = request.project.join("inhomogeneous-pair-correlation-store");
    fs::create_dir_all(&store_path).map_err(|source| MarklabError::io(&store_path, source))?;
    let store = LocalArtifactStore::open(
        &store_path,
        StoreId::new("inhomogeneous-pair-correlation-store")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let mut project = MarklabProject::with_inline_artifact_limit(memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
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
    let intensity_config = InhomogeneousSpatialConfig::new(
        request.radii_um,
        request.intensity_bandwidth_um,
        request.integration_grid,
        request.simulations,
        request.seed,
        request.alpha,
        request.minimum_intensity_per_um2,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config =
        InhomogeneousPairCorrelationConfig::new(intensity_config, request.pair_bandwidth_um)
            .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = InhomogeneousPairCorrelationAnalysisNode::new(
        &mut project,
        NodeId::new("inhomogeneous-pair-correlation")
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
        ArtifactSchema::new("marklab.inhomogeneous_pair_correlation", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = encode_pair_correlation_result(&run.output, &pattern, &window, &config)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&request.out)
        .map_err(|source| MarklabError::io(&request.out, source))?;
    output
        .write_all(&encoded)
        .map_err(|source| MarklabError::io(&request.out, source))?;
    output
        .sync_all()
        .map_err(|source| MarklabError::io(&request.out, source))?;
    let cache = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project inhomogeneous-pair-correlation cache_status={cache}");
    Ok(())
}

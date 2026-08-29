use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use crate::{
    execute_algorithm_with_store, inhomogeneous_spatial::encode_piecewise_compartment_result,
    ArtifactSchema, BinaryCompartmentPartition2D, CacheStatus, CompartmentPartitionLimits,
    CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    DurableProject, DurableProjectLimits, LocalArtifactStore, LocalScheduler, MarklabError,
    MarklabProject, NodeId, ObservationWindow2D, ObservationWindowLimits, PatternLoader,
    PiecewiseCompartmentSpatialAnalysisNode, PiecewiseCompartmentSpatialConfig,
    PiecewiseCompartmentSpatialLimits, Result, SchedulerLimits, SpatialAxis, StoreId, TumorMask,
    WorkflowGraph,
};

use super::classical::{native_runtime_provenance, read_bounded_utf8, source_artifact};

const SOURCE_CELLS_KIND: &str = "application/vnd.marklab.source.point-table;version=1";
const SOURCE_OBSERVATION_KIND: &str = "application/vnd.marklab.source.observation-window;version=1";
const SOURCE_NEGATIVE_KIND: &str =
    "application/vnd.marklab.source.negative-compartment-window;version=1";
const SOURCE_POSITIVE_KIND: &str =
    "application/vnd.marklab.source.positive-compartment-window;version=1";
const PROJECT_CONTROL_BYTES: usize = 1024 * 1024;
const PROJECT_LEDGER_BYTES: usize = 16 * 1024 * 1024;
const PROJECT_LEDGER_RECORDS: usize = 10_000;
const PROJECT_RECORD_BYTES: usize = 64 * 1024;
const FRAME_ID: &str = "piecewise-compartment-spatial-physical-xy-um";

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub observation_mask: PathBuf,
    pub negative_mask: PathBuf,
    pub positive_mask: PathBuf,
    pub negative_compartment_id: String,
    pub positive_compartment_id: String,
    pub out: PathBuf,
    pub radii_um: Vec<f64>,
    pub simulations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub memory_budget_mib: usize,
    pub maximum_boundary_segments: usize,
    pub maximum_compartment_queries: usize,
    pub maximum_pair_visits: usize,
    pub maximum_null_draws: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let sources_before = source_identities(&request)?;
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
    let observation_text =
        read_bounded_utf8(&request.observation_mask, window_limits.maximum_input_bytes)?;
    let negative_text =
        read_bounded_utf8(&request.negative_mask, window_limits.maximum_input_bytes)?;
    let positive_text =
        read_bounded_utf8(&request.positive_mask, window_limits.maximum_input_bytes)?;
    let mask = TumorMask::from_geojson_str(&observation_text)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let pattern = PatternLoader::new(&mask).load(&request.cells)?;
    let frame_id = CoordinateFrameId::new(FRAME_ID)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let registry = CoordinateRegistry::new(
        vec![CoordinateFrame::new(
            frame_id.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .map_err(|error| MarklabError::Validation(error.to_string()))?],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let parse_window = |text: &str| {
        ObservationWindow2D::from_geojson_str(text, window_limits)
            .map_err(|error| MarklabError::Geometry(error.to_string()))?
            .with_coordinate_frame(&registry, frame_id.clone())
            .map_err(|error| MarklabError::Geometry(error.to_string()))
    };
    let partition = BinaryCompartmentPartition2D::new(
        parse_window(&observation_text)?,
        request.negative_compartment_id.clone(),
        parse_window(&negative_text)?,
        request.positive_compartment_id.clone(),
        parse_window(&positive_text)?,
        CompartmentPartitionLimits::new(request.maximum_boundary_segments)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    if sources_before != source_identities(&request)? {
        return Err(MarklabError::Validation(
            "piecewise compartment source changed while its durable input was prepared".into(),
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
    let store_path = request.project.join("piecewise-compartment-spatial-store");
    fs::create_dir_all(&store_path).map_err(|source| MarklabError::io(&store_path, source))?;
    let store = LocalArtifactStore::open(
        &store_path,
        StoreId::new("piecewise-compartment-spatial-store")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let mut project = MarklabProject::with_inline_artifact_limit(memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let limits = PiecewiseCompartmentSpatialLimits::new(
        pattern.len(),
        request.radii_um.len(),
        request.maximum_compartment_queries,
        request.maximum_pair_visits,
        request.maximum_null_draws,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = PiecewiseCompartmentSpatialConfig::new(
        request.radii_um,
        request.simulations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = PiecewiseCompartmentSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("piecewise-compartment-spatial")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &pattern,
        &partition,
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
        ArtifactSchema::new("marklab.piecewise_compartment_spatial", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = encode_piecewise_compartment_result(&run.output, &pattern, &partition, &config)
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
    eprintln!("project piecewise-compartment-spatial cache_status={cache}");
    Ok(())
}

fn source_identities(request: &Request) -> Result<[crate::ArtifactRef; 4]> {
    Ok([
        source_artifact(&request.cells, SOURCE_CELLS_KIND)?,
        source_artifact(&request.observation_mask, SOURCE_OBSERVATION_KIND)?,
        source_artifact(&request.negative_mask, SOURCE_NEGATIVE_KIND)?,
        source_artifact(&request.positive_mask, SOURCE_POSITIVE_KIND)?,
    ])
}

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    compartments::{BinaryCompartmentPartition2D, CompartmentPartitionLimits},
    inhomogeneous_spatial::compartment_workflow::encode_piecewise_compartment_result,
    spatial::{
        PiecewiseCompartmentSpatialAnalysisNode, PiecewiseCompartmentSpatialConfig,
        PiecewiseCompartmentSpatialLimits,
    },
    workflow::{
        execute_algorithm_with_store, ArtifactSchema, DurableProject, DurableProjectLimits,
        LocalArtifactStore, LocalScheduler, MarklabProject, NodeId, SchedulerLimits, StoreId,
        WorkflowGraph,
    },
    CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    MarklabError, ObservationWindow2D, ObservationWindowLimits, Pattern, PatternLoader, Result,
    SpatialAxis, TumorMask,
};

pub(super) use super::project_output::write_output;
use super::project_runtime::{native_runtime_provenance, read_bounded_utf8, source_artifact};

const SOURCE_CELLS_KIND: &str = "application/vnd.marklab.source.point-table;version=1";
const SOURCE_OBSERVATION_KIND: &str = "application/vnd.marklab.source.observation-window;version=1";
const SOURCE_NEGATIVE_KIND: &str =
    "application/vnd.marklab.source.negative-compartment-window;version=1";
const SOURCE_POSITIVE_KIND: &str =
    "application/vnd.marklab.source.positive-compartment-window;version=1";
const FRAME_ID: &str = "piecewise-compartment-spatial-physical-xy-um";

#[derive(clap::Args, Debug)]
pub struct Request {
    /// Durable project directory that caches results.
    #[arg(long)]
    pub project: PathBuf,
    /// Cell table (CSV or Parquet).
    #[arg(long)]
    pub cells: PathBuf,
    /// Mask defining the observed tissue region.
    #[arg(long)]
    pub observation_mask: PathBuf,
    /// Mask defining the negative spatial compartment.
    #[arg(long)]
    pub negative_mask: PathBuf,
    /// Mask defining the positive spatial compartment.
    #[arg(long)]
    pub positive_mask: PathBuf,
    /// Identifier of the compartment treated as the negative level.
    #[arg(long)]
    pub negative_compartment_id: String,
    /// Identifier of the compartment treated as the positive level.
    #[arg(long)]
    pub positive_compartment_id: String,
    /// New JSON file for the result; it must not already exist.
    #[arg(long)]
    pub out: PathBuf,
    /// Comma-separated radii in µm.
    #[arg(long, value_delimiter = ',')]
    pub radii_um: Vec<f64>,
    /// Number of simulated null patterns.
    #[arg(long)]
    pub simulations: usize,
    /// Random seed; the same seed gives the same result.
    #[arg(long)]
    pub seed: u64,
    /// Significance level.
    #[arg(long)]
    pub alpha: f64,
    /// Memory budget in MiB.
    #[arg(long)]
    pub memory_budget_mib: usize,
    /// Upper bound on boundary segments.
    #[arg(long = "max-boundary-segments", value_name = "MAX_BOUNDARY_SEGMENTS")]
    pub maximum_boundary_segments: usize,
    /// Upper bound on compartment queries.
    #[arg(
        long = "max-compartment-queries",
        value_name = "MAX_COMPARTMENT_QUERIES"
    )]
    pub maximum_compartment_queries: usize,
    /// Maximum number of cell pairs visited.
    #[arg(long = "max-pair-visits", value_name = "MAX_PAIR_VISITS")]
    pub maximum_pair_visits: usize,
    /// Upper bound on null draws.
    #[arg(long = "max-null-draws", value_name = "MAX_NULL_DRAWS")]
    pub maximum_null_draws: usize,
}

pub(super) struct PrepareRequest<'a> {
    pub project: &'a Path,
    pub cells: &'a Path,
    pub observation_mask: &'a Path,
    pub negative_mask: &'a Path,
    pub positive_mask: &'a Path,
    pub negative_compartment_id: &'a str,
    pub positive_compartment_id: &'a str,
    pub memory_budget_mib: usize,
    pub maximum_boundary_segments: usize,
    pub store_id: &'static str,
}

pub(super) struct PreparedPiecewiseCompartmentProject {
    pub durable: DurableProject,
    pub project: MarklabProject,
    pub store: LocalArtifactStore,
    pub pattern: Pattern,
    pub partition: BinaryCompartmentPartition2D,
    pub memory_bytes: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let PreparedPiecewiseCompartmentProject {
        mut durable,
        mut project,
        store,
        pattern,
        partition,
        memory_bytes,
    } = prepare(PrepareRequest {
        project: &request.project,
        cells: &request.cells,
        observation_mask: &request.observation_mask,
        negative_mask: &request.negative_mask,
        positive_mask: &request.positive_mask,
        negative_compartment_id: &request.negative_compartment_id,
        positive_compartment_id: &request.positive_compartment_id,
        memory_budget_mib: request.memory_budget_mib,
        maximum_boundary_segments: request.maximum_boundary_segments,
        store_id: "piecewise-compartment-spatial-store",
    })?;
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
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "piecewise-compartment-spatial",
    )
}

pub(super) fn prepare(request: PrepareRequest<'_>) -> Result<PreparedPiecewiseCompartmentProject> {
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
        read_bounded_utf8(request.observation_mask, window_limits.maximum_input_bytes)?;
    let negative_text =
        read_bounded_utf8(request.negative_mask, window_limits.maximum_input_bytes)?;
    let positive_text =
        read_bounded_utf8(request.positive_mask, window_limits.maximum_input_bytes)?;
    let mask = TumorMask::from_geojson_str(&observation_text)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let pattern = PatternLoader::new(&mask).load(request.cells)?;
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
        request.negative_compartment_id,
        parse_window(&negative_text)?,
        request.positive_compartment_id,
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
    let durable_limits = DurableProjectLimits::for_object_bytes(memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let durable = DurableProject::open_or_create(request.project, durable_limits)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let store_path = request.project.join(request.store_id);
    fs::create_dir_all(&store_path).map_err(|source| MarklabError::io(&store_path, source))?;
    let store = LocalArtifactStore::open(
        &store_path,
        StoreId::new(request.store_id)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let project = MarklabProject::with_inline_artifact_limit(memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    Ok(PreparedPiecewiseCompartmentProject {
        durable,
        project,
        store,
        pattern,
        partition,
        memory_bytes,
    })
}

fn source_identities(request: &PrepareRequest<'_>) -> Result<[crate::workflow::ArtifactRef; 4]> {
    Ok([
        source_artifact(request.cells, SOURCE_CELLS_KIND)?,
        source_artifact(request.observation_mask, SOURCE_OBSERVATION_KIND)?,
        source_artifact(request.negative_mask, SOURCE_NEGATIVE_KIND)?,
        source_artifact(request.positive_mask, SOURCE_POSITIVE_KIND)?,
    ])
}

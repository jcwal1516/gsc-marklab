use std::path::PathBuf;

use crate::{
    inhomogeneous_spatial::compartment_g_workflow::encode_piecewise_compartment_pair_correlation_result,
    spatial::{
        PiecewiseCompartmentPairCorrelationAnalysisNode, PiecewiseCompartmentPairCorrelationConfig,
        PiecewiseCompartmentSpatialConfig, PiecewiseCompartmentSpatialLimits,
    },
    workflow::{
        execute_algorithm_with_store, ArtifactSchema, LocalScheduler, NodeId, SchedulerLimits,
        WorkflowGraph,
    },
    MarklabError, Result,
};

use super::{
    piecewise_compartment_spatial::{
        prepare, write_output, PrepareRequest, PreparedPiecewiseCompartmentProject,
    },
    project_runtime::native_runtime_provenance,
};

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
    /// Pair-correlation kernel bandwidth in µm.
    #[arg(long)]
    pub pair_bandwidth_um: f64,
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
        store_id: "piecewise-compartment-pair-correlation-store",
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
    let base = PiecewiseCompartmentSpatialConfig::new(
        request.radii_um,
        request.simulations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = PiecewiseCompartmentPairCorrelationConfig::new(base, request.pair_bandwidth_um)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = PiecewiseCompartmentPairCorrelationAnalysisNode::new(
        &mut project,
        NodeId::new("piecewise-compartment-pair-correlation")
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
        ArtifactSchema::new("marklab.piecewise_compartment_pair_correlation", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = encode_piecewise_compartment_pair_correlation_result(
        &run.output,
        &pattern,
        &partition,
        &config,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "piecewise-compartment-pair-correlation",
    )
}

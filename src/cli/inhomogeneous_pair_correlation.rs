use std::path::PathBuf;

use crate::{
    inhomogeneous_spatial::g_workflow::encode_pair_correlation_result,
    spatial::{
        InhomogeneousPairCorrelationAnalysisNode, InhomogeneousPairCorrelationConfig,
        InhomogeneousSpatialConfig, InhomogeneousSpatialLimits,
    },
    workflow::{
        execute_algorithm_with_store, ArtifactSchema, LocalScheduler, NodeId, SchedulerLimits,
        WorkflowGraph,
    },
    MarklabError, Result,
};

use super::{
    classical::native_runtime_provenance,
    inhomogeneous_project::{prepare, write_output, PrepareRequest, PreparedInhomogeneousProject},
};

#[derive(clap::Args, Debug)]
pub struct Request {
    /// Durable project directory that caches results.
    #[arg(long)]
    pub project: PathBuf,
    /// Cell table (CSV or Parquet).
    #[arg(long)]
    pub cells: PathBuf,
    /// Tissue mask (GeoJSON).
    #[arg(long)]
    pub mask: PathBuf,
    /// New JSON file for the result; it must not already exist.
    #[arg(long)]
    pub out: PathBuf,
    /// Comma-separated radii in µm.
    #[arg(long, value_delimiter = ',')]
    pub radii_um: Vec<f64>,
    /// Bandwidth of the estimated spatial intensity, in micrometers.
    #[arg(long)]
    pub intensity_bandwidth_um: f64,
    /// Pair-correlation kernel bandwidth in µm.
    #[arg(long)]
    pub pair_bandwidth_um: f64,
    /// Number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub grid_x: usize,
    /// Number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub grid_y: usize,
    /// Number of simulated null patterns.
    #[arg(long)]
    pub simulations: usize,
    /// Random seed; the same seed gives the same result.
    #[arg(long)]
    pub seed: u64,
    /// Significance level.
    #[arg(long)]
    pub alpha: f64,
    /// Lower bound on admitted intensity per square micrometer.
    #[arg(long)]
    pub minimum_intensity_per_um2: f64,
    /// Memory budget in MiB.
    #[arg(long)]
    pub memory_budget_mib: usize,
    /// Upper bound on probes.
    #[arg(long = "max-probes", value_name = "MAX_PROBES")]
    pub maximum_probes: usize,
    /// Upper bound on intensity evaluations.
    #[arg(
        long = "max-intensity-evaluations",
        value_name = "MAX_INTENSITY_EVALUATIONS"
    )]
    pub maximum_intensity_evaluations: usize,
    /// Maximum number of cell pairs visited.
    #[arg(long = "max-pair-visits", value_name = "MAX_PAIR_VISITS")]
    pub maximum_pair_visits: usize,
    /// Maximum number of points drawn for null simulations.
    #[arg(long = "max-null-draws", value_name = "MAX_NULL_DRAWS")]
    pub maximum_null_draws: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let integration_grid = [request.grid_x, request.grid_y];

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
        store_id: "inhomogeneous-pair-correlation-store",
        source_change_message:
            "inhomogeneous pair-correlation source changed while its durable input was prepared",
    })?;
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
        integration_grid,
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
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "inhomogeneous-pair-correlation",
    )
}

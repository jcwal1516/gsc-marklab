use std::path::PathBuf;

use crate::{
    spatial::{
        TranslationPairCorrelationAnalysisNode, TranslationPairCorrelationConfig,
        TranslationPairCorrelationResultDocument, TranslationSpatialLimits,
    },
    workflow::{
        execute_algorithm_with_store, ArtifactSchema, LocalScheduler, NodeId, SchedulerLimits,
        WorkflowGraph,
    },
    MarklabError, Result,
};

use super::{
    classical::{native_runtime_provenance, source_artifact},
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
    /// Kernel bandwidth in µm.
    #[arg(long)]
    pub bandwidth_um: f64,
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
    /// Maximum number of cell pairs visited.
    #[arg(long = "max-pair-visits", value_name = "MAX_PAIR_VISITS")]
    pub maximum_pair_visits: usize,
    /// Upper bound on overlap evaluations.
    #[arg(
        long = "max-overlap-evaluations",
        value_name = "MAX_OVERLAP_EVALUATIONS"
    )]
    pub maximum_overlap_evaluations: usize,
    /// Upper bound on overlap candidate work.
    #[arg(
        long = "max-overlap-candidate-work",
        value_name = "MAX_OVERLAP_CANDIDATE_WORK"
    )]
    pub maximum_overlap_candidate_work: usize,
    /// Upper bound on overlap output vertices.
    #[arg(
        long = "max-overlap-output-vertices",
        value_name = "MAX_OVERLAP_OUTPUT_VERTICES"
    )]
    pub maximum_overlap_output_vertices: usize,
    /// Maximum number of random points drawn for CSR simulations.
    #[arg(long = "max-csr-draws", value_name = "MAX_CSR_DRAWS")]
    pub maximum_csr_draws: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
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
        store_id: "translation-pair-correlation-store",
        source_change_message:
            "translation pair-correlation source changed while its durable input was prepared",
    })?;
    let limits = TranslationSpatialLimits::new(
        pattern.len().max(1),
        request.radii_um.len().max(1),
        request.maximum_pair_visits,
        request.maximum_overlap_evaluations,
        request.maximum_overlap_candidate_work,
        request.maximum_overlap_output_vertices,
        request.maximum_csr_draws,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = TranslationPairCorrelationConfig::new(
        request.radii_um,
        request.bandwidth_um,
        request.simulations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let runtime = native_runtime_provenance()?;
    let implementation_identity = format!(
        "{};adapter=translation-pair-correlation-analysis-node-v5;{}",
        runtime.implementation_identity(),
        crate::common::GEO_IDENTITY
    );
    let source_artifacts = vec![
        source_artifact(
            &request.cells,
            "application/vnd.marklab.source.point-table;version=1",
        )?,
        source_artifact(
            &request.mask,
            "application/vnd.marklab.source.observation-window;version=1",
        )?,
    ];
    let node = TranslationPairCorrelationAnalysisNode::new_with_implementation_identity(
        &mut project,
        NodeId::new("translation-pair-correlation")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &pattern,
        &window,
        &config,
        implementation_identity,
        source_artifacts,
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
        ArtifactSchema::new("marklab.translation_pair_correlation", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        runtime,
        &store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = TranslationPairCorrelationResultDocument::new(run.output)
        .and_then(|document| document.to_json_pretty())?
        .into_bytes();
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "translation-pair-correlation",
    )
}

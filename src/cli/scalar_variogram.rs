use std::path::PathBuf;

use crate::{
    autocorrelation::{
        ScalarVariogramAnalysisNode, ScalarVariogramBin, ScalarVariogramInferenceDesign,
        ScalarVariogramInferenceLimits,
    },
    marks::{DeclaredScalarPatternInput, ScalarMarkId},
    workflow::{
        execute_algorithm_with_store, ArtifactSchema, LocalScheduler, NodeId, SchedulerLimits,
        WorkflowGraph, WorkflowNode,
    },
    MarklabError, Result,
};

use super::{
    categorical_mark_project::{prepare_nucleus_area, write_output, PrepareRequest},
    classical::native_runtime_provenance,
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
    /// Distance-bin edges in micrometers.
    #[arg(long, value_delimiter = ',')]
    pub lag_edges_um: Vec<f64>,
    /// Restrict comparisons to the declared histologic compartments.
    #[arg(long)]
    pub condition_by_histologic_compartment: bool,
    /// Number of label permutations.
    #[arg(long)]
    pub permutations: usize,
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
    /// Upper bound on permutation pair evaluations.
    #[arg(
        long = "max-permutation-pair-evaluations",
        value_name = "MAX_PERMUTATION_PAIR_EVALUATIONS"
    )]
    pub maximum_permutation_pair_evaluations: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let bins = bins(&request.lag_edges_um)?;
    let mut prepared = prepare_nucleus_area(PrepareRequest {
        project: &request.project,
        cells: &request.cells,
        mask: &request.mask,
        memory_budget_mib: request.memory_budget_mib,
        store_id: "scalar-variogram-store",
    })?;
    let input = DeclaredScalarPatternInput::from_mark_table(
        &prepared.project,
        &prepared.pattern,
        &prepared.table,
        prepared.slide_id.clone(),
        prepared.frame_id.clone(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let design = if request.condition_by_histologic_compartment {
        ScalarVariogramInferenceDesign::histologic_compartment_random_labeling(
            request.permutations,
            request.seed,
            request.alpha,
        )
    } else {
        ScalarVariogramInferenceDesign::random_labeling(
            request.permutations,
            request.seed,
            request.alpha,
        )
    }
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let limits = ScalarVariogramInferenceLimits::new(
        prepared.pattern.len(),
        request.maximum_pair_visits,
        request.maximum_permutation_pair_evaluations,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let mark_id = ScalarMarkId::new("nucleus_area_um2")
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = ScalarVariogramAnalysisNode::new(
        &mut prepared.project,
        NodeId::new("scalar-variogram")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &input,
        &prepared.window,
        &mark_id,
        &bins,
        &design,
        limits,
        prepared.memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: prepared.memory_bytes,
    })
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let run = execute_algorithm_with_store(
        &mut prepared.durable,
        &mut prepared.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.scalar_variogram_inference", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &prepared.store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = node
        .encode_output(&run.output)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(&request.out, &encoded, run.cache_status, "scalar-variogram")
}

fn bins(edges: &[f64]) -> Result<Vec<ScalarVariogramBin>> {
    if edges.len() < 2 {
        return Err(MarklabError::Validation(
            "--lag-edges-um requires at least two edges".into(),
        ));
    }
    edges
        .windows(2)
        .map(|edge| {
            ScalarVariogramBin::new(edge[0], edge[1])
                .map_err(|error| MarklabError::Validation(error.to_string()))
        })
        .collect()
}

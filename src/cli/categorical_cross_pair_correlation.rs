use std::path::PathBuf;

use crate::{
    cross_pair_correlation::{
        isotropic_workflow::encode_result as encode_isotropic_result,
        translation_workflow::encode_result as encode_translation_result, workflow::encode_result,
    },
    marks::{CategoricalPairLimits, DeclaredScalarPatternInput},
    spatial::{
        CategoricalCrossPairCorrelationAnalysisNode, CategoricalCrossPairCorrelationConfig,
        IsotropicCategoricalCrossPairCorrelationAnalysisNode,
        IsotropicCategoricalCrossPairCorrelationConfig, IsotropicSpatialLimits,
        TranslationCategoricalCrossPairCorrelationAnalysisNode,
        TranslationCategoricalCrossPairCorrelationConfig, TranslationSpatialLimits,
    },
    workflow::{
        execute_algorithm_with_store, ArtifactSchema, LocalScheduler, NodeId, SchedulerLimits,
        WorkflowGraph,
    },
    MarklabError, Result,
};

use super::{
    categorical_mark_project::{prepare, write_output, PrepareRequest},
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
    /// Tissue mask (GeoJSON).
    #[arg(long)]
    pub mask: PathBuf,
    /// New JSON file for the result; it must not already exist.
    #[arg(long)]
    pub out: PathBuf,
    /// Compartment level of the source cells.
    #[arg(long)]
    pub source_level: String,
    /// Compartment level of the target cells.
    #[arg(long)]
    pub target_level: String,
    /// Comma-separated radii in µm.
    #[arg(long, value_delimiter = ',')]
    pub radii_um: Vec<f64>,
    /// Kernel bandwidth in µm.
    #[arg(long)]
    pub bandwidth_um: f64,
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
    /// Maximum number of pair evaluations across null replicates.
    #[arg(
        long = "max-null-pair-evaluations",
        value_name = "MAX_NULL_PAIR_EVALUATIONS"
    )]
    pub maximum_null_pair_evaluations: usize,
}

#[derive(clap::Args, Debug)]
pub struct TranslationRequest {
    #[command(flatten)]
    pub base: Request,
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
}

#[derive(clap::Args, Debug)]
pub struct IsotropicRequest {
    #[command(flatten)]
    pub base: Request,
    /// Upper bound on visible arc evaluations.
    #[arg(
        long = "max-visible-arc-evaluations",
        value_name = "MAX_VISIBLE_ARC_EVALUATIONS"
    )]
    pub maximum_visible_arc_evaluations: usize,
    /// Upper bound on arc segment tests.
    #[arg(long = "max-arc-segment-tests", value_name = "MAX_ARC_SEGMENT_TESTS")]
    pub maximum_arc_segment_tests: usize,
    /// Upper bound on arc membership queries.
    #[arg(
        long = "max-arc-membership-queries",
        value_name = "MAX_ARC_MEMBERSHIP_QUERIES"
    )]
    pub maximum_arc_membership_queries: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let mut prepared = prepare(PrepareRequest {
        project: &request.project,
        cells: &request.cells,
        mask: &request.mask,
        memory_budget_mib: request.memory_budget_mib,
        store_id: "categorical-cross-g-store",
    })?;
    let input = DeclaredScalarPatternInput::from_mark_table(
        &prepared.project,
        &prepared.pattern,
        &prepared.table,
        prepared.slide_id.clone(),
        prepared.frame_id.clone(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let limits = CategoricalPairLimits::new(
        prepared.pattern.len(),
        request.radii_um.len(),
        request.maximum_pair_visits,
        request.maximum_null_pair_evaluations,
        prepared.memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = CategoricalCrossPairCorrelationConfig::new(
        request.radii_um,
        request.bandwidth_um,
        request.source_level,
        request.target_level,
        request.permutations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = CategoricalCrossPairCorrelationAnalysisNode::new(
        &mut prepared.project,
        NodeId::new("categorical-cross-pair-correlation")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &input,
        &prepared.window,
        &config,
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
        ArtifactSchema::new("marklab.categorical_cross_pair_correlation", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &prepared.store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = encode_result(&run.output, &input, &prepared.window, &config)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "categorical-cross-pair-correlation",
    )
}

pub(super) fn run_translation_project(request: TranslationRequest) -> Result<()> {
    let mut prepared = prepare(PrepareRequest {
        project: &request.base.project,
        cells: &request.base.cells,
        mask: &request.base.mask,
        memory_budget_mib: request.base.memory_budget_mib,
        store_id: "translation-categorical-cross-g-store",
    })?;
    let input = DeclaredScalarPatternInput::from_mark_table(
        &prepared.project,
        &prepared.pattern,
        &prepared.table,
        prepared.slide_id.clone(),
        prepared.frame_id.clone(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let categorical_limits = CategoricalPairLimits::new(
        prepared.pattern.len(),
        request.base.radii_um.len(),
        request.base.maximum_pair_visits,
        request.base.maximum_null_pair_evaluations,
        prepared.memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let base = CategoricalCrossPairCorrelationConfig::new(
        request.base.radii_um,
        request.base.bandwidth_um,
        request.base.source_level,
        request.base.target_level,
        request.base.permutations,
        request.base.seed,
        request.base.alpha,
        categorical_limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let translation_limits = TranslationSpatialLimits::new(
        prepared.pattern.len().max(1),
        base.radii_um().len().max(1),
        request.base.maximum_pair_visits,
        request.maximum_overlap_evaluations,
        request.maximum_overlap_candidate_work,
        request.maximum_overlap_output_vertices,
        1,
        prepared.memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = TranslationCategoricalCrossPairCorrelationConfig::new(base, translation_limits)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = TranslationCategoricalCrossPairCorrelationAnalysisNode::new(
        &mut prepared.project,
        NodeId::new("translation-categorical-cross-pair-correlation")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &input,
        &prepared.window,
        &config,
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
        ArtifactSchema::new("marklab.translation_categorical_cross_g", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &prepared.store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = encode_translation_result(&run.output, &config)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(
        &request.base.out,
        &encoded,
        run.cache_status,
        "translation-categorical-cross-pair-correlation",
    )
}

pub(super) fn run_isotropic_project(request: IsotropicRequest) -> Result<()> {
    let mut prepared = prepare(PrepareRequest {
        project: &request.base.project,
        cells: &request.base.cells,
        mask: &request.base.mask,
        memory_budget_mib: request.base.memory_budget_mib,
        store_id: "isotropic-categorical-cross-g-store",
    })?;
    let input = DeclaredScalarPatternInput::from_mark_table(
        &prepared.project,
        &prepared.pattern,
        &prepared.table,
        prepared.slide_id.clone(),
        prepared.frame_id.clone(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let categorical_limits = CategoricalPairLimits::new(
        prepared.pattern.len(),
        request.base.radii_um.len(),
        request.base.maximum_pair_visits,
        request.base.maximum_null_pair_evaluations,
        prepared.memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let base = CategoricalCrossPairCorrelationConfig::new(
        request.base.radii_um,
        request.base.bandwidth_um,
        request.base.source_level,
        request.base.target_level,
        request.base.permutations,
        request.base.seed,
        request.base.alpha,
        categorical_limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let isotropic_limits = IsotropicSpatialLimits::new(
        prepared.pattern.len().max(1),
        base.radii_um().len().max(1),
        request.base.maximum_pair_visits,
        request.maximum_visible_arc_evaluations,
        request.maximum_arc_segment_tests,
        request.maximum_arc_membership_queries,
        1,
        prepared.memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = IsotropicCategoricalCrossPairCorrelationConfig::new(base, isotropic_limits)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = IsotropicCategoricalCrossPairCorrelationAnalysisNode::new(
        &mut prepared.project,
        NodeId::new("isotropic-categorical-cross-pair-correlation")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &input,
        &prepared.window,
        &config,
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
        ArtifactSchema::new("marklab.isotropic_categorical_cross_g", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &prepared.store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = encode_isotropic_result(&run.output, &config)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(
        &request.base.out,
        &encoded,
        run.cache_status,
        "isotropic-categorical-cross-pair-correlation",
    )
}

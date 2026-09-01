use std::path::PathBuf;

use crate::{
    execute_algorithm_with_store,
    inhomogeneous_spatial::categorical_cross_g_workflow::encode_result as encode_categorical_cross_g_result,
    ArtifactSchema, DeclaredScalarPatternInput,
    InhomogeneousCategoricalCrossPairCorrelationAnalysisNode,
    InhomogeneousCategoricalCrossPairCorrelationConfig, InhomogeneousSpatialConfig,
    InhomogeneousSpatialLimits, LocalScheduler, MarklabError, NodeId, Result, SchedulerLimits,
    WorkflowGraph,
};

use super::{
    categorical_mark_project::{prepare, write_output, PrepareRequest},
    classical::native_runtime_provenance,
};

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub mask: PathBuf,
    pub out: PathBuf,
    pub source_level: String,
    pub target_level: String,
    pub radii_um: Vec<f64>,
    pub intensity_bandwidth_um: f64,
    pub cross_fit_folds: Option<usize>,
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
    let mut prepared = prepare(PrepareRequest {
        project: &request.project,
        cells: &request.cells,
        mask: &request.mask,
        memory_budget_mib: request.memory_budget_mib,
        store_id: "inhomogeneous-categorical-cross-g-store",
    })?;
    let input = DeclaredScalarPatternInput::from_mark_table(
        &prepared.project,
        &prepared.pattern,
        &prepared.table,
        prepared.slide_id.clone(),
        prepared.frame_id.clone(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let limits = InhomogeneousSpatialLimits::new(
        prepared.pattern.len(),
        request.radii_um.len(),
        request.maximum_probes,
        request.maximum_intensity_evaluations,
        request.maximum_pair_visits,
        request.maximum_null_draws,
        prepared.memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let intensity = if let Some(folds) = request.cross_fit_folds {
        InhomogeneousSpatialConfig::new_cross_fitted(
            request.radii_um,
            request.intensity_bandwidth_um,
            request.integration_grid,
            folds,
            request.simulations,
            request.seed,
            request.alpha,
            request.minimum_intensity_per_um2,
            limits,
        )
    } else {
        InhomogeneousSpatialConfig::new(
            request.radii_um,
            request.intensity_bandwidth_um,
            request.integration_grid,
            request.simulations,
            request.seed,
            request.alpha,
            request.minimum_intensity_per_um2,
            limits,
        )
    }
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = InhomogeneousCategoricalCrossPairCorrelationConfig::new(
        intensity,
        request.pair_bandwidth_um,
        request.source_level,
        request.target_level,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = InhomogeneousCategoricalCrossPairCorrelationAnalysisNode::new(
        &mut prepared.project,
        NodeId::new("inhomogeneous-categorical-cross-pair-correlation")
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
        ArtifactSchema::new(
            "marklab.inhomogeneous_categorical_cross_pair_correlation",
            1,
        )
        .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &prepared.store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = encode_categorical_cross_g_result(&run.output, &input, &prepared.window, &config)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    write_output(
        &request.out,
        &encoded,
        run.cache_status,
        "inhomogeneous-categorical-cross-pair-correlation",
    )
}

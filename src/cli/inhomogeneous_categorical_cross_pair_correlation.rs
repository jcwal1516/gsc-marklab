use super::schema::CliInhomogeneousCrossEdgeCorrection;
use std::path::PathBuf;

use crate::{
    inhomogeneous_spatial::categorical_cross_g_workflow::encode_result as encode_categorical_cross_g_result,
    marks::DeclaredScalarPatternInput,
    spatial::{
        InhomogeneousCategoricalCrossPairCorrelationAnalysisNode,
        InhomogeneousCategoricalCrossPairCorrelationConfig, InhomogeneousSpatialConfig,
        InhomogeneousSpatialLimits, IsotropicSpatialLimits, TranslationSpatialLimits,
    },
    workflow::{
        execute_algorithm_with_store, ArtifactSchema, LocalScheduler, NodeId, SchedulerLimits,
        WorkflowGraph,
    },
    MarklabError, Result,
};

use super::{
    categorical_mark_project::{prepare, write_output, PrepareRequest},
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
    /// Compartment level of the source cells.
    #[arg(long)]
    pub source_level: String,
    /// Compartment level of the target cells.
    #[arg(long)]
    pub target_level: String,
    /// Comma-separated radii in µm.
    #[arg(long, value_delimiter = ',')]
    pub radii_um: Vec<f64>,
    /// Bandwidth of the estimated spatial intensity, in micrometers.
    #[arg(long)]
    pub intensity_bandwidth_um: f64,
    /// Number of folds for cross-fitted intensity estimation.
    #[arg(long)]
    pub cross_fit_folds: Option<usize>,
    /// Boundary correction used by the spatial estimator.
    #[arg(long, value_enum, default_value_t = CliInhomogeneousCrossEdgeCorrection::StandardBorder)]
    pub edge_correction: CliInhomogeneousCrossEdgeCorrection,
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
    /// Upper bound on overlap evaluations.
    #[arg(
        long = "max-overlap-evaluations",
        value_name = "MAX_OVERLAP_EVALUATIONS"
    )]
    pub maximum_overlap_evaluations: Option<usize>,
    /// Upper bound on overlap candidate work.
    #[arg(
        long = "max-overlap-candidate-work",
        value_name = "MAX_OVERLAP_CANDIDATE_WORK"
    )]
    pub maximum_overlap_candidate_work: Option<usize>,
    /// Upper bound on overlap output vertices.
    #[arg(
        long = "max-overlap-output-vertices",
        value_name = "MAX_OVERLAP_OUTPUT_VERTICES"
    )]
    pub maximum_overlap_output_vertices: Option<usize>,
    /// Upper bound on visible arc evaluations.
    #[arg(
        long = "max-visible-arc-evaluations",
        value_name = "MAX_VISIBLE_ARC_EVALUATIONS"
    )]
    pub maximum_visible_arc_evaluations: Option<usize>,
    /// Upper bound on arc segment tests.
    #[arg(long = "max-arc-segment-tests", value_name = "MAX_ARC_SEGMENT_TESTS")]
    pub maximum_arc_segment_tests: Option<usize>,
    /// Upper bound on arc membership queries.
    #[arg(
        long = "max-arc-membership-queries",
        value_name = "MAX_ARC_MEMBERSHIP_QUERIES"
    )]
    pub maximum_arc_membership_queries: Option<usize>,
}

pub(super) enum EdgeCorrectionRequest {
    StandardBorder,
    Translation {
        maximum_overlap_evaluations: usize,
        maximum_overlap_candidate_work: usize,
        maximum_overlap_output_vertices: usize,
    },
    Isotropic {
        maximum_visible_arc_evaluations: usize,
        maximum_arc_segment_tests: usize,
        maximum_arc_membership_queries: usize,
    },
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let required = |value: Option<usize>, name: &str| {
        value.ok_or_else(|| {
            crate::MarklabError::Validation(format!(
                "--{name} is required by the selected inhomogeneous edge correction"
            ))
        })
    };
    let edge_correction = match request.edge_correction {
        CliInhomogeneousCrossEdgeCorrection::StandardBorder => {
            EdgeCorrectionRequest::StandardBorder
        }
        CliInhomogeneousCrossEdgeCorrection::Translation => EdgeCorrectionRequest::Translation {
            maximum_overlap_evaluations: required(
                request.maximum_overlap_evaluations,
                "max-overlap-evaluations",
            )?,
            maximum_overlap_candidate_work: required(
                request.maximum_overlap_candidate_work,
                "max-overlap-candidate-work",
            )?,
            maximum_overlap_output_vertices: required(
                request.maximum_overlap_output_vertices,
                "max-overlap-output-vertices",
            )?,
        },
        CliInhomogeneousCrossEdgeCorrection::Isotropic => EdgeCorrectionRequest::Isotropic {
            maximum_visible_arc_evaluations: required(
                request.maximum_visible_arc_evaluations,
                "max-visible-arc-evaluations",
            )?,
            maximum_arc_segment_tests: required(
                request.maximum_arc_segment_tests,
                "max-arc-segment-tests",
            )?,
            maximum_arc_membership_queries: required(
                request.maximum_arc_membership_queries,
                "max-arc-membership-queries",
            )?,
        },
    };
    let integration_grid = [request.grid_x, request.grid_y];

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
            integration_grid,
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
            integration_grid,
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
    let radius_count = config.intensity_config().radii_um().len();
    let config = match edge_correction {
        EdgeCorrectionRequest::StandardBorder => config,
        EdgeCorrectionRequest::Translation {
            maximum_overlap_evaluations,
            maximum_overlap_candidate_work,
            maximum_overlap_output_vertices,
        } => config
            .with_translation_correction(
                TranslationSpatialLimits::new(
                    prepared.pattern.len(),
                    radius_count,
                    request.maximum_pair_visits,
                    maximum_overlap_evaluations,
                    maximum_overlap_candidate_work,
                    maximum_overlap_output_vertices,
                    1,
                    prepared.memory_bytes,
                )
                .map_err(|error| MarklabError::Validation(error.to_string()))?,
            )
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        EdgeCorrectionRequest::Isotropic {
            maximum_visible_arc_evaluations,
            maximum_arc_segment_tests,
            maximum_arc_membership_queries,
        } => config
            .with_isotropic_correction(
                IsotropicSpatialLimits::new(
                    prepared.pattern.len(),
                    radius_count,
                    request.maximum_pair_visits,
                    maximum_visible_arc_evaluations,
                    maximum_arc_segment_tests,
                    maximum_arc_membership_queries,
                    1,
                    prepared.memory_bytes,
                )
                .map_err(|error| MarklabError::Validation(error.to_string()))?,
            )
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
    };
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

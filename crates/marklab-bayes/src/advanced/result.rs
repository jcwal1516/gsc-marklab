use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde::{Deserialize, Serialize};
use serde_json::Number;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HmcNormalResult {
    pub format: String,
    pub version: u32,
    pub algorithm: HmcAlgorithm,
    pub posterior: HmcPosterior,
    pub analytic_posterior: AnalyticPosterior,
    pub diagnostics: HmcDiagnostics,
    pub claim_status: ClaimStatus,
    pub backend: Backend,
    pub request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HmcAlgorithm {
    pub mass: f64,
    pub step_size: f64,
    pub leapfrog_steps: usize,
    pub warmup: usize,
    pub draws: usize,
    pub seed: Number,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HmcPosterior {
    pub mean: f64,
    pub standard_deviation: f64,
    pub interval_95: [f64; 2],
    pub draws: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyticPosterior {
    pub mean: f64,
    pub standard_deviation: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HmcDiagnostics {
    pub acceptance_rate: f64,
    pub maximum_absolute_energy_error: f64,
    pub mean_energy_error: f64,
    pub constraint_transform_jacobian: f64,
}

/// Earlier `bayes advanced-cluster` output, `marklab.advanced_cluster_and_gibbs_models` version 2:
/// a k-means cluster-count heuristic, a finite binary Gibbs exchange fit and fixed shrinkage.
/// It is no longer produced; [`super::thomas_cluster::ClusterModelDocument`] still reads it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedClusterResult {
    pub format: String,
    pub version: u32,
    pub cluster_heuristic: ClusterHeuristic,
    pub exchange_mcmc: ExchangeMcmc,
    pub fixed_log_shrinkage: FixedLogShrinkage,
    pub limitations: Vec<String>,
    pub claim_status: ClaimStatus,
    pub backend: Backend,
    pub request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterHeuristic {
    pub method: String,
    pub score_definition: String,
    pub mean_cluster_count: f64,
    pub mean_within_cluster_scale: f64,
    pub mean_points_per_cluster: f64,
    pub pattern_results: Vec<ClusterPatternResult>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterPatternResult {
    pub pattern_id: String,
    pub patient_id: String,
    pub cluster_count_mean: f64,
    pub cluster_count_interval_95: [f64; 2],
    pub within_cluster_scale: f64,
    pub clusters_per_unit_area: f64,
    pub points_per_cluster: f64,
    pub count_sampler_acceptance_rate: f64,
    pub boundary_policy: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeMcmc {
    pub posterior_interaction_mean: f64,
    pub posterior_interaction_interval_95: [f64; 2],
    pub acceptance_rate: f64,
    pub observed_same_type_edge_statistic: f64,
    pub state_space_size: usize,
    pub auxiliary_draw_status: String,
    pub draws: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixedLogShrinkage {
    pub patient_count: usize,
    pub shrinkage: f64,
    pub population_log_parameter_mean: [f64; 3],
    pub patient_log_parameter_means: Vec<[f64; 3]>,
    pub parameters: [String; 3],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularSpdeResult {
    pub format: String,
    pub version: u32,
    pub mesh: RectangularMesh,
    pub lgcp: LgcpSummary,
    pub spatial_factor: RectangularSpatialFactor,
    pub mesh_sensitivity: Vec<RectangularMeshSensitivity>,
    pub limitations: Vec<String>,
    pub claim_status: ClaimStatus,
    pub backend: Backend,
    pub request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularMesh {
    pub frame: String,
    pub bounds: [Number; 4],
    pub holes: Vec<Vec<[Number; 2]>>,
    pub boundary_condition: String,
    pub basis: String,
    pub alpha: u32,
    pub kappa: f64,
    pub tau: f64,
    pub resolution: usize,
    pub vertices: Vec<[f64; 2]>,
    pub triangles: Vec<[usize; 3]>,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub mass_matrix: SparseTriplets,
    pub stiffness_matrix: SparseTriplets,
    pub precision_matrix: SparseTriplets,
    pub minimum_precision_eigenvalue: f64,
    pub maximum_projection_row_sum_error: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LgcpSummary {
    pub inference: String,
    pub beta: f64,
    pub latent_field_weights: Vec<f64>,
    pub vertex_intensity: Vec<f64>,
    pub integrated_intensity: f64,
    pub right_to_left_mean_intensity_ratio: f64,
    pub objective: f64,
    pub gradient_maximum: f64,
    pub iterations: usize,
    pub event_projection: SparseTriplets,
    pub quadrature_projection: SparseTriplets,
    pub quadrature_weights: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularSpatialFactor {
    pub inference: String,
    pub factor_count: u32,
    pub feature_means: Vec<f64>,
    pub mesh_field_weights: Vec<f64>,
    pub projected_region_factor: Vec<f64>,
    pub loadings: Vec<f64>,
    pub reconstruction_rmse: f64,
    pub factor_x_correlation: f64,
    pub objective: f64,
    pub iterations: usize,
    pub projection: SparseTriplets,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularMeshSensitivity {
    pub resolution: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub integrated_intensity: f64,
    pub right_to_left_mean_intensity_ratio: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonstationaryAdaptiveWindowSpdeResult {
    pub format: String,
    pub version: u32,
    pub statistical_unit: StatisticalUnit,
    pub window: NonstationaryWindowSummary,
    pub mesh: NonstationaryMesh,
    pub background: LocalParameterSummary,
    pub parameter_regions: Vec<LocalParameterSummary>,
    pub spatial_factor: NonstationarySpatialFactor,
    pub limits: NonstationaryLimits,
    pub assumptions: Vec<String>,
    pub limitations: Vec<String>,
    pub claim_status: ClaimStatus,
    pub backend: Backend,
    pub request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonstationaryWindowSummary {
    pub frame: String,
    pub exact_area: f64,
    pub bounds: [f64; 4],
    pub component_count: usize,
    pub hole_count: usize,
    pub ring_count: usize,
    pub vertex_count: usize,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonstationaryMesh {
    pub operator: String,
    pub basis: String,
    pub boundary_condition: String,
    pub alpha: u32,
    pub vertices: Vec<[f64; 2]>,
    pub triangles: Vec<[usize; 3]>,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub mesh_area: f64,
    pub relative_area_error: f64,
    pub mass_matrix: SparseTriplets,
    pub anisotropic_stiffness_matrix: SparseTriplets,
    pub precision_matrix: SparseTriplets,
    pub minimum_precision_eigenvalue: f64,
    pub maximum_precision_eigenvalue: f64,
    pub maximum_precision_symmetry_error: f64,
    pub maximum_projection_row_sum_error: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalParameterSummary {
    pub region_id: String,
    pub kappa: Number,
    pub tau: Number,
    pub anisotropy_ratio: Number,
    pub major_axis_degrees: Number,
    pub anisotropy_tensor: [[f64; 2]; 2],
    pub interior_refinement_levels: Number,
    pub triangle_count: usize,
    pub median_triangle_area: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonstationarySpatialFactor {
    pub inference: String,
    pub factor_count: u32,
    pub region_ids: Vec<String>,
    pub feature_count: usize,
    pub feature_means: Vec<f64>,
    pub mesh_field_weights: Vec<f64>,
    pub projected_region_factor: Vec<f64>,
    pub loadings: Vec<f64>,
    pub reconstruction_rmse: f64,
    pub objective: f64,
    pub gradient_maximum: f64,
    pub iterations: usize,
    pub projection: SparseTriplets,
    pub projection_triangle_visits: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonstationaryLimits {
    pub maximum_vertices: Number,
    pub maximum_triangles: Number,
    pub maximum_candidate_points: Number,
    pub maximum_boundary_segment_triangle_checks: Number,
    pub maximum_projection_triangle_visits: Number,
    pub memory_budget_mib: Number,
    pub maximum_result_bytes: Number,
    pub maximum_iterations: Number,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveWindowSpdeResult {
    pub format: String,
    pub version: u32,
    pub window: AdaptiveWindowSummary,
    pub mesh: AdaptiveMeshSummary,
    pub spatial_factor: AdaptiveSpatialFactorSummary,
    pub mesh_sensitivity: Vec<AdaptiveMeshSensitivity>,
    pub limits: AdaptiveSpdeLimits,
    pub assumptions: Vec<String>,
    pub limitations: Vec<String>,
    pub claim_status: ClaimStatus,
    pub backend: Backend,
    pub request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveWindowSummary {
    pub frame: String,
    pub exact_area: f64,
    pub bounds: [f64; 4],
    pub component_count: usize,
    pub hole_count: usize,
    pub ring_count: usize,
    pub vertex_count: usize,
    pub geometry_policy: String,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveMeshSummary {
    pub basis: String,
    pub boundary_condition: String,
    pub alpha: u32,
    pub kappa: f64,
    pub tau: f64,
    pub base_resolution: u32,
    pub boundary_refinement_levels: u32,
    pub vertices: Vec<[f64; 2]>,
    pub triangles: Vec<[usize; 3]>,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub connected_components: usize,
    pub mesh_area: f64,
    pub relative_area_error: f64,
    pub constant_mass_integral: f64,
    pub mass_matrix: SparseTriplets,
    pub stiffness_matrix: SparseTriplets,
    pub precision_matrix: SparseTriplets,
    pub minimum_precision_eigenvalue: f64,
    pub maximum_precision_eigenvalue: f64,
    pub maximum_projection_row_sum_error: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseTriplets {
    pub rows: Vec<usize>,
    pub columns: Vec<usize>,
    pub values: Vec<f64>,
    pub shape: [usize; 2],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveSpatialFactorSummary {
    pub inference: String,
    pub factor_count: u32,
    pub region_ids: Vec<String>,
    pub feature_count: usize,
    pub feature_means: Vec<f64>,
    pub mesh_field_weights: Vec<f64>,
    pub projected_region_factor: Vec<f64>,
    pub loadings: Vec<f64>,
    pub reconstruction_rmse: f64,
    pub factor_x_correlation: f64,
    pub objective: f64,
    pub gradient_maximum: f64,
    pub iterations: usize,
    pub projection: SparseTriplets,
    pub projection_triangle_visits: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveMeshSensitivity {
    pub boundary_refinement_level: u32,
    pub boundary_spacing: f64,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub mesh_area: f64,
    pub relative_area_error: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveSpdeLimits {
    pub maximum_vertices: usize,
    pub maximum_triangles: usize,
    pub maximum_boundary_segment_triangle_checks: u64,
    pub maximum_projection_triangle_visits: u64,
    pub memory_budget_mib: usize,
    pub maximum_result_bytes: usize,
    pub maximum_iterations: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Backend {
    pub name: String,
    pub scipy_version: String,
    pub numpy_version: String,
    pub python_version: String,
    pub license: String,
    pub environment_lock_sha256: String,
    pub worker_sha256: String,
}

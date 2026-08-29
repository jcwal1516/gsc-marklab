#![forbid(unsafe_code)]

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const GUDHI_VERSION: &str = "3.13.0";

mod connectivity;
pub use connectivity::{
    connectivity_transition, ConnectivityCurvePoint, ConnectivityPointInput,
    ConnectivityTransitionResult, ConnectivityTransitionSpec, ConnectivityWindow,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TopologyPointInput {
    pub id: String,
    pub coordinates_um: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaPersistenceSpec {
    pub points: Vec<TopologyPointInput>,
    pub max_alpha_um: f64,
    pub maximum_dimension: usize,
    pub coefficient_field: u32,
    pub transform_dimension: usize,
    pub landscape_grid_alpha_squared: Vec<f64>,
    pub landscape_max_k: usize,
    pub image_birth_edges_alpha_squared: Vec<f64>,
    pub image_persistence_edges_alpha_squared: Vec<f64>,
    pub image_kernel_bandwidth_alpha_squared: f64,
    pub euler_thresholds_alpha_squared: Vec<f64>,
    pub maximum_simplices: usize,
    pub timeout_seconds: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LandmarkMethod {
    FarthestPoint,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessPersistenceSpec {
    pub points: Vec<TopologyPointInput>,
    pub landmark_method: LandmarkMethod,
    pub landmark_count: usize,
    pub maximum_dimension: usize,
    pub nu: usize,
    pub max_scale_um: f64,
    pub coefficient_field: u32,
    pub maximum_simplices: usize,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TopologyBackendContract {
    pub name: String,
    pub version: String,
    pub python_version: String,
    pub license: String,
    pub environment_lock_sha256: String,
    pub worker_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct AlphaPersistenceWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: TopologyBackendContract,
    pub points: Vec<TopologyPointInput>,
    pub max_alpha_square: f64,
    pub maximum_dimension: usize,
    pub coefficient_field: u32,
    pub transform_dimension: usize,
    pub landscape_grid_alpha_squared: Vec<f64>,
    pub landscape_max_k: usize,
    pub image_birth_edges_alpha_squared: Vec<f64>,
    pub image_persistence_edges_alpha_squared: Vec<f64>,
    pub image_kernel_bandwidth_alpha_squared: f64,
    pub euler_thresholds_alpha_squared: Vec<f64>,
    pub maximum_simplices: usize,
}

#[derive(Debug, Serialize)]
pub struct WitnessPersistenceWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: TopologyBackendContract,
    pub points: Vec<TopologyPointInput>,
    pub landmark_method: LandmarkMethod,
    pub landmark_count: usize,
    pub maximum_dimension: usize,
    pub nu: usize,
    pub max_scale_square: f64,
    pub coefficient_field: u32,
    pub maximum_simplices: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaSimplex {
    pub vertex_ids: Vec<String>,
    pub dimension: usize,
    pub filtration_alpha_squared: f64,
    pub boundary_indices: Vec<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaFiltrationResult {
    pub convention: String,
    pub precision: String,
    pub coefficient_field: u32,
    pub tie_break_order: String,
    pub validation_status: String,
    pub simplices: Vec<AlphaSimplex>,
    pub simplex_counts_by_dimension: Vec<usize>,
    pub boundary_of_boundary_max_abs: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistencePair {
    pub birth: f64,
    pub death: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistenceDimensionResult {
    pub dimension: usize,
    pub finite_pairs: Vec<PersistencePair>,
    pub essential_births: Vec<f64>,
    pub essential_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistenceResult {
    pub algorithm: String,
    pub essential_death_policy: String,
    pub by_dimension: Vec<PersistenceDimensionResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistenceLandscapeResult {
    pub dimension: usize,
    pub grid_alpha_squared: Vec<f64>,
    pub max_k: usize,
    pub norm_convention: String,
    pub values: Vec<Vec<f64>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistenceImageResult {
    pub dimension: usize,
    pub birth_edges_alpha_squared: Vec<f64>,
    pub persistence_edges_alpha_squared: Vec<f64>,
    pub kernel_bandwidth_alpha_squared: f64,
    pub weight: String,
    pub normalization: String,
    pub integration: String,
    pub values: Vec<Vec<f64>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EulerCurveResult {
    pub thresholds_alpha_squared: Vec<f64>,
    pub values: Vec<i64>,
    pub simplex_counts_by_threshold: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaPersistenceResult {
    pub format: String,
    pub version: u32,
    pub backend: TopologyBackendContract,
    pub request_sha256: String,
    pub points: Vec<TopologyPointInput>,
    pub max_alpha_um: f64,
    pub filtration: AlphaFiltrationResult,
    pub persistence: PersistenceResult,
    pub landscape: PersistenceLandscapeResult,
    pub persistence_image: PersistenceImageResult,
    pub euler_curve: EulerCurveResult,
    pub claim_status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessApproximationResult {
    pub landmark_count: usize,
    pub coverage_radius_um: f64,
    pub nu: usize,
    pub max_scale_um: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessPersistenceResult {
    pub format: String,
    pub version: u32,
    pub backend: TopologyBackendContract,
    pub request_sha256: String,
    pub points: Vec<TopologyPointInput>,
    pub landmark_ids: Vec<String>,
    pub approximation: WitnessApproximationResult,
    pub filtration: AlphaFiltrationResult,
    pub persistence: PersistenceResult,
    pub claim_status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessPersistenceStabilitySpec {
    pub points: Vec<TopologyPointInput>,
    pub landmark_method: LandmarkMethod,
    pub landmark_count: usize,
    pub maximum_dimension: usize,
    pub nu: usize,
    pub max_scale_um: f64,
    pub coefficient_field: u32,
    pub maximum_simplices: usize,
    pub timeout_seconds: u64,
    pub perturbation_replicates: usize,
    pub maximum_coordinate_jitter_um: f64,
    pub seed: u64,
    pub minimum_landmark_id_match_fraction: f64,
    pub maximum_coverage_radius_change_um: f64,
    pub maximum_simplex_count_l1_change: u64,
    pub maximum_total_persistence_change_um_squared: f64,
    pub maximum_backend_executions: u64,
    pub maximum_total_point_work: u64,
    pub maximum_total_simplex_budget: u64,
    pub maximum_total_timeout_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessDimensionStabilityResult {
    pub dimension: usize,
    pub finite_pair_count_change: u64,
    pub essential_count_change: u64,
    pub total_persistence_change_um_squared: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessPerturbationStabilityResult {
    pub replicate: usize,
    pub request_sha256: String,
    pub maximum_coordinate_displacement_um: f64,
    pub landmark_id_match_fraction: f64,
    pub coverage_radius_change_um: f64,
    pub simplex_count_l1_change: u64,
    pub by_dimension: Vec<WitnessDimensionStabilityResult>,
    pub maximum_total_persistence_change_um_squared: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessPersistenceStabilityResult {
    pub format: String,
    pub version: u32,
    pub statistical_unit: String,
    pub perturbation_rule: String,
    pub finite_result_policy: String,
    pub seed: u64,
    pub perturbation_replicates: usize,
    pub maximum_coordinate_jitter_um: f64,
    pub minimum_landmark_id_match_fraction_allowed: f64,
    pub maximum_coverage_radius_change_um_allowed: f64,
    pub maximum_simplex_count_l1_change_allowed: u64,
    pub maximum_total_persistence_change_um_squared_allowed: f64,
    pub baseline: WitnessPersistenceResult,
    pub perturbations: Vec<WitnessPerturbationStabilityResult>,
    pub minimum_landmark_id_match_fraction: f64,
    pub maximum_coverage_radius_change_um: f64,
    pub maximum_simplex_count_l1_change: u64,
    pub maximum_total_persistence_change_um_squared: f64,
    pub backend_executions: u64,
    pub total_point_work: u64,
    pub total_simplex_budget: u64,
    pub total_timeout_seconds: u64,
    pub stable_under_declared_thresholds: bool,
    pub claim_status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessPersistenceBottleneckStabilitySpec {
    pub stability: WitnessPersistenceStabilitySpec,
    pub maximum_bottleneck_distance_um_squared: f64,
    pub maximum_bottleneck_comparisons: u64,
    pub maximum_bottleneck_interval_budget: u64,
    pub maximum_bottleneck_backend_executions: u64,
    pub bottleneck_timeout_seconds: u64,
    pub maximum_total_backend_executions: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessBottleneckDimensionResult {
    pub dimension: usize,
    pub status: String,
    pub bottleneck_distance_um_squared: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessBottleneckPerturbationResult {
    pub replicate: usize,
    pub by_dimension: Vec<WitnessBottleneckDimensionResult>,
    pub maximum_finite_bottleneck_distance_um_squared: f64,
    pub has_infinite_essential_mismatch: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessPersistenceBottleneckStabilityResult {
    pub format: String,
    pub version: u32,
    pub backend: TopologyBackendContract,
    pub bottleneck_request_sha256: String,
    pub bottleneck_metric: String,
    pub essential_interval_policy: String,
    pub stability: WitnessPersistenceStabilityResult,
    pub perturbations: Vec<WitnessBottleneckPerturbationResult>,
    pub maximum_bottleneck_distance_um_squared_allowed: f64,
    pub maximum_finite_bottleneck_distance_um_squared: f64,
    pub has_infinite_essential_mismatch: bool,
    pub bottleneck_comparisons: u64,
    pub bottleneck_interval_count: u64,
    pub bottleneck_backend_executions: u64,
    pub total_backend_executions: u64,
    pub stable_under_bottleneck_threshold: bool,
    pub stable_under_all_declared_thresholds: bool,
    pub claim_status: String,
}

#[derive(Debug, Error)]
pub enum TopologyError {
    #[error("invalid topology specification: {0}")]
    Invalid(String),
    #[error("invalid topology backend result: {0}")]
    Backend(String),
}

impl AlphaPersistenceWorkerRequest {
    pub fn new(
        mut spec: AlphaPersistenceSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, TopologyError> {
        validate_spec(&spec)?;
        spec.points.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self {
            format: "marklab.gudhi_alpha_persistence_request",
            version: 1,
            backend: TopologyBackendContract {
                name: "gudhi".into(),
                version: GUDHI_VERSION.into(),
                python_version: "3.12".into(),
                license: "MIT_with_GPLv3_CGAL_alpha_complex_dependency".into(),
                environment_lock_sha256,
                worker_sha256,
            },
            points: spec.points,
            max_alpha_square: spec.max_alpha_um.powi(2),
            maximum_dimension: spec.maximum_dimension,
            coefficient_field: spec.coefficient_field,
            transform_dimension: spec.transform_dimension,
            landscape_grid_alpha_squared: spec.landscape_grid_alpha_squared,
            landscape_max_k: spec.landscape_max_k,
            image_birth_edges_alpha_squared: spec.image_birth_edges_alpha_squared,
            image_persistence_edges_alpha_squared: spec.image_persistence_edges_alpha_squared,
            image_kernel_bandwidth_alpha_squared: spec.image_kernel_bandwidth_alpha_squared,
            euler_thresholds_alpha_squared: spec.euler_thresholds_alpha_squared,
            maximum_simplices: spec.maximum_simplices,
        })
    }
}

impl WitnessPersistenceWorkerRequest {
    pub fn new(
        mut spec: WitnessPersistenceSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, TopologyError> {
        if !(3..=10_000).contains(&spec.points.len())
            || !(2..spec.points.len()).contains(&spec.landmark_count)
            || !matches!(spec.maximum_dimension, 1..=3)
            || spec.nu != 0
            || !spec.max_scale_um.is_finite()
            || spec.max_scale_um <= 0.0
            || !matches!(spec.coefficient_field, 2 | 3 | 5 | 7 | 11 | 13 | 17 | 19)
            || !(3..=1_000_000).contains(&spec.maximum_simplices)
            || !(1..=3_600).contains(&spec.timeout_seconds)
        {
            return Err(TopologyError::Invalid(
                "witness dimensions, landmark count, nu, field, scale, bounds, or timeout are invalid"
                    .into(),
            ));
        }
        let coordinate_dimension = spec.points[0].coordinates_um.len();
        let mut ids = HashSet::new();
        if coordinate_dimension == 0
            || coordinate_dimension > 3
            || spec.points.iter().any(|point| {
                point.id.trim().is_empty()
                    || point.id.trim() != point.id
                    || !ids.insert(point.id.as_str())
                    || point.coordinates_um.len() != coordinate_dimension
                    || point.coordinates_um.iter().any(|value| !value.is_finite())
            })
        {
            return Err(TopologyError::Invalid(
                "witness points require unique exact IDs and common finite one-to-three-dimensional coordinates"
                    .into(),
            ));
        }
        spec.points.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self {
            format: "marklab.gudhi_witness_persistence_request",
            version: 1,
            backend: TopologyBackendContract {
                name: "gudhi".into(),
                version: GUDHI_VERSION.into(),
                python_version: "3.12".into(),
                license: "MIT_with_GPLv3_CGAL_alpha_complex_dependency".into(),
                environment_lock_sha256,
                worker_sha256,
            },
            points: spec.points,
            landmark_method: spec.landmark_method,
            landmark_count: spec.landmark_count,
            maximum_dimension: spec.maximum_dimension,
            nu: spec.nu,
            max_scale_square: spec.max_scale_um.powi(2),
            coefficient_field: spec.coefficient_field,
            maximum_simplices: spec.maximum_simplices,
        })
    }
}

impl AlphaPersistenceResult {
    pub fn parse_and_validate(
        bytes: &[u8],
        request: &AlphaPersistenceWorkerRequest,
        request_bytes: &[u8],
    ) -> Result<Self, TopologyError> {
        let result: Self = serde_json::from_slice(bytes)
            .map_err(|error| TopologyError::Backend(error.to_string()))?;
        if result.format != "marklab.alpha_persistence"
            || result.version != 1
            || result.backend.name != request.backend.name
            || result.backend.version != request.backend.version
            || result.backend.python_version != request.backend.python_version
            || result.backend.license != request.backend.license
            || result.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || result.backend.worker_sha256 != request.backend.worker_sha256
            || result.request_sha256 != sha256_hex(request_bytes)
            || result.filtration.validation_status != "passed"
            || result.filtration.coefficient_field != request.coefficient_field
            || result.filtration.simplices.len() > request.maximum_simplices
            || result.landscape.dimension != request.transform_dimension
            || result.persistence_image.dimension != request.transform_dimension
            || result.claim_status != "experimental_synthetic_alpha_topology"
        {
            return Err(TopologyError::Backend(
                "topology result identity or contract mismatch".into(),
            ));
        }
        let finite = result
            .filtration
            .simplices
            .iter()
            .all(|simplex| simplex.filtration_alpha_squared.is_finite())
            && result.persistence.by_dimension.iter().all(|dimension| {
                dimension.finite_pairs.iter().all(|pair| {
                    pair.birth.is_finite() && pair.death.is_finite() && pair.death >= pair.birth
                }) && dimension
                    .essential_births
                    .iter()
                    .all(|birth| birth.is_finite())
            })
            && result
                .landscape
                .values
                .iter()
                .flatten()
                .chain(result.persistence_image.values.iter().flatten())
                .all(|value| value.is_finite());
        if !finite || result.filtration.boundary_of_boundary_max_abs > 1e-12 {
            return Err(TopologyError::Backend(
                "topology result contains non-finite values or invalid boundaries".into(),
            ));
        }
        Ok(result)
    }
}

impl WitnessPersistenceResult {
    pub fn parse_and_validate(
        bytes: &[u8],
        request: &WitnessPersistenceWorkerRequest,
        request_bytes: &[u8],
    ) -> Result<Self, TopologyError> {
        let result: Self = serde_json::from_slice(bytes)
            .map_err(|error| TopologyError::Backend(error.to_string()))?;
        result.validate(request, request_bytes)?;
        Ok(result)
    }

    pub fn validate(
        &self,
        request: &WitnessPersistenceWorkerRequest,
        request_bytes: &[u8],
    ) -> Result<(), TopologyError> {
        let known_ids = request
            .points
            .iter()
            .map(|point| point.id.as_str())
            .collect::<HashSet<_>>();
        if self.format != "marklab.witness_persistence"
            || self.version != 1
            || self.backend.name != request.backend.name
            || self.backend.version != request.backend.version
            || self.backend.python_version != request.backend.python_version
            || self.backend.license != request.backend.license
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != sha256_hex(request_bytes)
            || self.landmark_ids.len() != request.landmark_count
            || self
                .landmark_ids
                .iter()
                .any(|id| !known_ids.contains(id.as_str()))
            || self.approximation.landmark_count != request.landmark_count
            || self.approximation.nu != request.nu
            || !self.approximation.coverage_radius_um.is_finite()
            || self.filtration.validation_status != "passed"
            || self.filtration.simplices.len() > request.maximum_simplices
            || self.claim_status != "experimental_witness_approximation"
        {
            return Err(TopologyError::Backend(
                "witness result identity or contract mismatch".into(),
            ));
        }
        Ok(())
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_spec(spec: &AlphaPersistenceSpec) -> Result<(), TopologyError> {
    if !(3..=10_000).contains(&spec.points.len())
        || !spec.max_alpha_um.is_finite()
        || spec.max_alpha_um <= 0.0
        || !matches!(spec.maximum_dimension, 1..=3)
        || spec.transform_dimension > spec.maximum_dimension
        || !matches!(spec.coefficient_field, 2 | 3 | 5 | 7 | 11 | 13 | 17 | 19)
        || !(1..=8).contains(&spec.landscape_max_k)
        || !(7..=1_000_000).contains(&spec.maximum_simplices)
        || !(1..=3_600).contains(&spec.timeout_seconds)
        || !spec.image_kernel_bandwidth_alpha_squared.is_finite()
        || spec.image_kernel_bandwidth_alpha_squared <= 0.0
    {
        return Err(TopologyError::Invalid(
            "alpha topology dimensions, field, bounds, bandwidth, or timeout are invalid".into(),
        ));
    }
    let coordinate_dimension = spec.points[0].coordinates_um.len();
    let mut ids = HashSet::new();
    if coordinate_dimension != spec.maximum_dimension
        || spec.points.iter().any(|point| {
            point.id.trim().is_empty()
                || point.id.trim() != point.id
                || !ids.insert(point.id.as_str())
                || point.coordinates_um.len() != coordinate_dimension
                || point.coordinates_um.iter().any(|value| !value.is_finite())
        })
    {
        return Err(TopologyError::Invalid(
            "alpha points require unique exact IDs and finite coordinates matching maximum_dimension"
                .into(),
        ));
    }
    for values in [
        &spec.landscape_grid_alpha_squared,
        &spec.image_birth_edges_alpha_squared,
        &spec.image_persistence_edges_alpha_squared,
        &spec.euler_thresholds_alpha_squared,
    ] {
        if values.len() < 2
            || values.len() > 4_096
            || values.iter().any(|value| !value.is_finite())
            || values.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(TopologyError::Invalid(
                "topology grids/edges/thresholds must be finite, bounded, and strictly increasing"
                    .into(),
            ));
        }
    }
    Ok(())
}

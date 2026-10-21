//! Strict wire contracts for the existing 3-D research workers.

use std::collections::{BTreeMap, HashSet};

use marklab_core::backend::is_worker_timeout;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::{ValidationStatus, ValidationSuiteStatus};
use serde::{Deserialize, Serialize};
use serde_json::Number;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScipyBackend {
    pub name: String,
    pub scipy_version: String,
    pub numpy_version: String,
    pub python_version: String,
    pub license: String,
    pub environment_lock_sha256: String,
    pub worker_sha256: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AlphaBackend {
    pub name: String,
    pub version: String,
    pub numpy_version: String,
    pub python_version: String,
    pub license: String,
    pub environment_lock_sha256: String,
    pub worker_sha256: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationBackend {
    pub name: String,
    pub gudhi_version: String,
    pub scipy_version: String,
    pub numpy_version: String,
    pub python_version: String,
    pub license: String,
    pub environment_lock_sha256: String,
    pub worker_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SerialStackRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: ScipyBackend,
    pub stack_id: &'a str,
    pub coordinate_unit: &'a str,
    pub sections: &'a [SerialSection],
    pub reference_section_id: &'a str,
    pub landmark_noise_standard_deviation_um: f64,
    pub posterior_draws: usize,
    pub seed: u64,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaComplexRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: AlphaBackend,
    pub coordinate_frame: &'a str,
    pub points: &'a [AlphaPoint3d],
    pub maximum_squared_alpha_um2: f64,
    pub maximum_homology_dimension: usize,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeformationBiologyRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: ScipyBackend,
    pub coordinate_frame: &'a str,
    pub points: &'a [DeformationBiologyPoint],
    pub deformation_translation_draws: &'a [[f64; 2]],
    pub change_model: &'a str,
    pub interpolation: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CloneModelsRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: ScipyBackend,
    pub tree: &'a CloneTree,
    pub clone_labels: &'a [String],
    pub clone_locations: &'a [CloneLocation],
    pub cells: &'a [CloneCell],
    pub diffusion_prior: &'a DiffusionPrior,
    pub posterior_draws: usize,
    pub seed: u64,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedValidationRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: ValidationBackend,
    pub seed: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SerialStackResult {
    pub format: String,
    pub version: u32,
    pub backend: ScipyBackend,
    pub request_sha256: String,
    pub stack_id: String,
    pub coordinate_unit: String,
    pub reference_section_id: String,
    pub ordered_sections: Vec<OrderedSection>,
    pub pairwise_models: Vec<PairwiseModel>,
    pub stack_posterior: StackPosterior,
    pub transformed_landmarks_xyz_um: Vec<Vec<[f64; 3]>>,
    pub quality: StackQuality,
    pub downstream_uncertainty: StackUncertainty,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrderedSection {
    pub section_id: String,
    pub z_um: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PairwiseModel {
    pub source_section_id: String,
    pub target_section_id: String,
    pub translation_xy_um: [f64; 2],
    pub landmark_rmse_um: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StackPosterior {
    pub family: String,
    pub draw_count: usize,
    pub section_translations_xy_um: Vec<SectionTranslation>,
    pub reference_constraint: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SectionTranslation {
    pub section_id: String,
    pub z_um: f64,
    pub posterior_mean: [f64; 2],
    pub posterior_standard_deviation: [f64; 2],
    pub draws: Vec<[f64; 2]>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StackQuality {
    pub landmark_rmse_um: f64,
    pub maximum_cycle_consistency_error_um: f64,
    pub section_gap_um: Vec<f64>,
    pub missing_section_model: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StackUncertainty {
    pub analysis: String,
    pub stack_centroid_mean_um: [f64; 3],
    pub stack_centroid_standard_deviation_um: [f64; 3],
    pub transform_draw_outputs_um: Vec<[f64; 3]>,
    pub between_transform_uncertainty: String,
    pub biological_sampling_uncertainty: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaComplexResult {
    pub format: String,
    pub version: u32,
    pub backend: AlphaBackend,
    pub request_sha256: String,
    pub coordinate_frame: String,
    pub filtration_convention: String,
    pub point_ids: Vec<String>,
    pub maximum_simplex_dimension: i32,
    pub simplex_counts_by_dimension: [usize; 4],
    pub filtration: Vec<AlphaSimplex>,
    pub tetrahedra: Vec<AlphaSimplex>,
    pub persistence_diagrams: BTreeMap<u32, Vec<AlphaInterval>>,
    pub boundary_squared_max_absolute: u32,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaSimplex {
    pub vertices: Vec<String>,
    pub vertex_indices: Vec<usize>,
    pub dimension: usize,
    pub squared_alpha_um2: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaInterval {
    pub birth_squared_alpha_um2: f64,
    #[serde(deserialize_with = "Option::deserialize")]
    pub death_squared_alpha_um2: Option<f64>,
    pub essential: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeformationBiologyResult {
    pub format: String,
    pub version: u32,
    pub backend: ScipyBackend,
    pub request_sha256: String,
    pub coordinate_frame: String,
    pub deformation: DeformationSummary,
    pub biological_change: BiologicalChange,
    pub negative_control: DeformationNegativeControl,
    pub independent_measurement: IndependentMeasurement,
    pub identifiability: DeformationIdentifiability,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeformationSummary {
    pub family: String,
    pub draw_count: usize,
    pub translation_xy_mean: [f64; 2],
    pub translation_xy_standard_deviation: [f64; 2],
    pub interpolation: String,
    pub mean_baseline_interpolation_rmse: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BiologicalChange {
    pub model: String,
    pub intercept_mean: f64,
    pub domain_effect_mean: f64,
    pub domain_effect_standard_deviation: f64,
    pub domain_effect_lower_95: f64,
    pub domain_effect_upper_95: f64,
    pub point_change_mean: Vec<f64>,
    pub point_change_standard_deviation: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeformationNegativeControl {
    pub domain_effect_mean: f64,
    pub absolute_domain_effect_mean: f64,
    pub domain_effect_standard_deviation: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IndependentMeasurement {
    pub correlation: f64,
    pub role: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeformationIdentifiability {
    pub deformation_posterior_supplied: bool,
    pub negative_control_supplied: bool,
    pub independent_change_measurement_supplied: bool,
    pub sampling_change_model: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CloneModelsResult {
    pub format: String,
    pub version: u32,
    pub backend: ScipyBackend,
    pub request_sha256: String,
    pub tree: CloneTree,
    pub clone_labels: Vec<String>,
    pub phylogeography: Phylogeography,
    pub niche_model: NicheModel,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Phylogeography {
    pub transition: String,
    pub diffusion_posterior_mean: f64,
    pub diffusion_posterior_standard_deviation: f64,
    pub diffusion_lower_95: f64,
    pub diffusion_upper_95: f64,
    pub diffusion_draws: Vec<f64>,
    pub posterior_shape_mean: f64,
    pub posterior_scale_mean: f64,
    pub location_uncertainty_integrated: bool,
    pub direction_or_history_identified: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NicheModel {
    pub likelihood: String,
    pub patient_count: usize,
    pub cell_count: usize,
    pub clone_a_minus_b_contrast: Vec<f64>,
    pub clone_a_minus_b_standard_error: Vec<f64>,
    pub patient_feature_means: Vec<Vec<f64>>,
    pub objective: Vec<f64>,
    pub leave_one_patient_out_contrasts: Vec<Vec<f64>>,
    pub leave_one_patient_out_maximum_contrast_change: Vec<f64>,
    pub assignment_uncertainty_integrated: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedValidationResult {
    pub format: String,
    pub version: u32,
    pub backend: ValidationBackend,
    pub request_sha256: String,
    pub seed: u64,
    pub overall_status: ValidationSuiteStatus,
    pub entries: Vec<ValidationEntry>,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationEntry {
    pub validation_id: String,
    pub status: ValidationStatus,
    pub metric: String,
    pub value: ValidationMeasurement,
    pub threshold: ValidationThreshold,
    pub evidence: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ValidationMeasurement {
    Number(Number),
    Flag(bool),
    Pair([Number; 2]),
    Missing(()),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ValidationThreshold {
    Description(String),
    Flag(bool),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SerialSection {
    pub section_id: String,
    pub z_um: f64,
    pub observed_landmarks: Vec<[f64; 2]>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerialStackSpec {
    pub stack_id: String,
    pub coordinate_unit: String,
    pub sections: Vec<SerialSection>,
    pub reference_section_id: String,
    pub landmark_noise_standard_deviation_um: f64,
    pub posterior_draws: usize,
    pub seed: u64,
    pub timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaPoint3d {
    pub point_id: String,
    pub coordinates_um: [f64; 3],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaComplex3dSpec {
    pub coordinate_frame: String,
    pub points: Vec<AlphaPoint3d>,
    pub maximum_squared_alpha_um2: f64,
    pub maximum_homology_dimension: usize,
    pub timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeformationBiologyPoint {
    pub point_id: String,
    pub pre_coordinates: [f64; 2],
    pub pre_value: f64,
    pub post_coordinates: [f64; 2],
    pub post_value: f64,
    pub negative_control_post_value: f64,
    pub domain_indicator: f64,
    pub independent_change_measurement: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeformationBiologySpec {
    pub coordinate_frame: String,
    pub points: Vec<DeformationBiologyPoint>,
    pub deformation_translation_draws: Vec<[f64; 2]>,
    pub change_model: String,
    pub interpolation: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CloneBranch {
    pub parent: String,
    pub child: String,
    pub length: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CloneTree {
    pub tree_id: String,
    pub root_id: String,
    pub nodes: Vec<String>,
    pub branches: Vec<CloneBranch>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CloneLocation {
    pub clone_id: String,
    pub mean_xy_um: [f64; 2],
    pub covariance_um2: [[f64; 2]; 2],
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CloneCell {
    pub cell_id: String,
    pub patient_id: String,
    pub coordinates_um: [f64; 2],
    pub clone_probabilities: Vec<f64>,
    pub neighborhood_features: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionPrior {
    pub inverse_gamma_shape: f64,
    pub inverse_gamma_scale: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloneModelsSpec {
    pub tree: CloneTree,
    pub clone_labels: Vec<String>,
    pub clone_locations: Vec<CloneLocation>,
    pub cells: Vec<CloneCell>,
    pub diffusion_prior: DiffusionPrior,
    pub posterior_draws: usize,
    pub seed: u64,
    pub timeout_seconds: u64,
}

pub fn validate_clone_models(spec: &CloneModelsSpec) -> Result<(), String> {
    let mut nodes = HashSet::new();
    let valid_nodes =
        spec.tree.nodes.iter().all(|node| {
            !node.trim().is_empty() && node.trim() == node && nodes.insert(node.as_str())
        });
    let mut children = HashSet::new();
    let valid_branches = spec.tree.branches.iter().all(|branch| {
        nodes.contains(branch.parent.as_str())
            && nodes.contains(branch.child.as_str())
            && branch.parent != branch.child
            && children.insert(branch.child.as_str())
            && branch.length.is_finite()
            && branch.length > 0.0
    });
    let mut labels = HashSet::new();
    let valid_labels = spec.clone_labels.iter().all(|label| {
        !label.trim().is_empty() && label.trim() == label && labels.insert(label.as_str())
    });
    let mut location_ids = HashSet::new();
    let valid_locations = spec.clone_locations.iter().all(|location| {
        let covariance = location.covariance_um2;
        let determinant = covariance[0][0] * covariance[1][1] - covariance[0][1] * covariance[1][0];
        nodes.contains(location.clone_id.as_str())
            && location_ids.insert(location.clone_id.as_str())
            && location.mean_xy_um.iter().all(|value| value.is_finite())
            && covariance.iter().flatten().all(|value| value.is_finite())
            && covariance[0][1] == covariance[1][0]
            && covariance[0][0] > 0.0
            && determinant > 0.0
    });
    let feature_count = spec
        .cells
        .first()
        .map(|cell| cell.neighborhood_features.len())
        .unwrap_or(0);
    let mut cell_ids = HashSet::new();
    let mut patients = HashSet::new();
    let valid_cells = spec.cells.iter().all(|cell| {
        patients.insert(cell.patient_id.as_str());
        !cell.cell_id.trim().is_empty()
            && cell.cell_id.trim() == cell.cell_id
            && cell_ids.insert(cell.cell_id.as_str())
            && !cell.patient_id.trim().is_empty()
            && cell.coordinates_um.iter().all(|value| value.is_finite())
            && cell.clone_probabilities.len() == spec.clone_labels.len()
            && cell
                .clone_probabilities
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0)
            && (cell.clone_probabilities.iter().sum::<f64>() - 1.0).abs() <= 1e-12
            && cell.neighborhood_features.len() == feature_count
            && cell
                .neighborhood_features
                .iter()
                .all(|value| value.is_finite())
    });
    if spec.tree.tree_id.trim().is_empty()
        || !(3..=128).contains(&spec.tree.nodes.len())
        || !valid_nodes
        || !nodes.contains(spec.tree.root_id.as_str())
        || spec.tree.branches.len() != spec.tree.nodes.len() - 1
        || !valid_branches
        || children.contains(spec.tree.root_id.as_str())
        || children.len() != spec.tree.nodes.len() - 1
        || spec.clone_labels.len() != 2
        || !valid_labels
        || !spec
            .clone_labels
            .iter()
            .all(|label| nodes.contains(label.as_str()))
        || spec.clone_locations.len() != spec.tree.nodes.len()
        || !valid_locations
        || location_ids.len() != nodes.len()
        || !(40..=100_000).contains(&spec.cells.len())
        || !(1..=32).contains(&feature_count)
        || !valid_cells
        || patients.len() < 4
        || !spec.diffusion_prior.inverse_gamma_shape.is_finite()
        || spec.diffusion_prior.inverse_gamma_shape <= 1.0
        || !spec.diffusion_prior.inverse_gamma_scale.is_finite()
        || spec.diffusion_prior.inverse_gamma_scale <= 0.0
        || !(100..=2_000).contains(&spec.posterior_draws)
        || !is_worker_timeout(spec.timeout_seconds)
    {
        return Err(
            "clone models require one rooted imported tree, SPD location uncertainty for every node, two clone labels, normalized uncertain assignments, replicated patients/features, and bounded posterior draws".into(),
        );
    }
    let mut reached = HashSet::from([spec.tree.root_id.as_str()]);
    let mut pending = vec![spec.tree.root_id.as_str()];
    while let Some(parent) = pending.pop() {
        for branch in spec
            .tree
            .branches
            .iter()
            .filter(|branch| branch.parent == parent)
        {
            if reached.insert(branch.child.as_str()) {
                pending.push(branch.child.as_str());
            }
        }
    }
    if reached.len() != nodes.len() {
        return Err(
            "clone branches must form one rooted tree with every node reachable from its root"
                .into(),
        );
    }
    Ok(())
}

pub fn validate_deformation_biology(spec: &DeformationBiologySpec) -> Result<(), String> {
    let mut point_ids = HashSet::new();
    let mut pre_coordinates = HashSet::new();
    let mut post_coordinates = HashSet::new();
    let valid_points = spec.points.iter().all(|point| {
        !point.point_id.trim().is_empty()
            && point.point_id.trim() == point.point_id
            && point_ids.insert(point.point_id.as_str())
            && point
                .pre_coordinates
                .iter()
                .chain(&point.post_coordinates)
                .all(|value| value.is_finite())
            && pre_coordinates.insert((
                point.pre_coordinates[0].to_bits(),
                point.pre_coordinates[1].to_bits(),
            ))
            && post_coordinates.insert((
                point.post_coordinates[0].to_bits(),
                point.post_coordinates[1].to_bits(),
            ))
            && [
                point.pre_value,
                point.post_value,
                point.negative_control_post_value,
                point.domain_indicator,
                point.independent_change_measurement,
            ]
            .iter()
            .all(|value| value.is_finite())
            && matches!(point.domain_indicator, 0.0 | 1.0)
    });
    if spec.coordinate_frame.trim().is_empty()
        || spec.coordinate_frame.trim() != spec.coordinate_frame
        || !(16..=10_000).contains(&spec.points.len())
        || !valid_points
        || !spec
            .points
            .iter()
            .any(|point| point.domain_indicator == 0.0)
        || !spec
            .points
            .iter()
            .any(|point| point.domain_indicator == 1.0)
        || !(100..=2_000).contains(&spec.deformation_translation_draws.len())
        || spec
            .deformation_translation_draws
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        || spec.change_model != "intercept_plus_prespecified_domain"
        || spec.interpolation != "thin_plate_spline"
        || !is_worker_timeout(spec.timeout_seconds)
    {
        return Err(
            "deformation-biology separation requires paired finite fields, prespecified binary domain, deformation posterior draws, negative control, independent measurement, and bounded interpolation work".into(),
        );
    }
    Ok(())
}

pub fn validate_alpha_complex(spec: &AlphaComplex3dSpec) -> Result<(), String> {
    let mut point_ids = HashSet::new();
    let mut coordinates = HashSet::new();
    let valid_points = spec.points.iter().all(|point| {
        !point.point_id.trim().is_empty()
            && point.point_id.trim() == point.point_id
            && point_ids.insert(point.point_id.as_str())
            && point.coordinates_um.iter().all(|value| value.is_finite())
            && coordinates.insert((
                point.coordinates_um[0].to_bits(),
                point.coordinates_um[1].to_bits(),
                point.coordinates_um[2].to_bits(),
            ))
    });
    if spec.coordinate_frame.trim().is_empty()
        || spec.coordinate_frame.trim() != spec.coordinate_frame
        || !(4..=128).contains(&spec.points.len())
        || !valid_points
        || !spec.maximum_squared_alpha_um2.is_finite()
        || spec.maximum_squared_alpha_um2 <= 0.0
        || spec.maximum_homology_dimension > 3
        || !is_worker_timeout(spec.timeout_seconds)
    {
        return Err(
            "3-D alpha complexes require unique finite physical points/frame, positive squared alpha, bounded homology dimension, and bounded exact work".into(),
        );
    }
    Ok(())
}

pub fn validate_serial_stack(spec: &SerialStackSpec) -> Result<(), String> {
    let landmark_count = spec
        .sections
        .first()
        .map(|section| section.observed_landmarks.len())
        .unwrap_or(0);
    let mut section_ids = HashSet::new();
    let valid_sections = spec.sections.iter().all(|section| {
        let mut landmarks = HashSet::new();
        !section.section_id.trim().is_empty()
            && section.section_id.trim() == section.section_id
            && section_ids.insert(section.section_id.as_str())
            && section.z_um.is_finite()
            && section.observed_landmarks.len() == landmark_count
            && section.observed_landmarks.iter().all(|point| {
                point.iter().all(|value| value.is_finite())
                    && landmarks.insert((point[0].to_bits(), point[1].to_bits()))
            })
    });
    if spec.stack_id.trim().is_empty()
        || spec.stack_id.trim() != spec.stack_id
        || spec.coordinate_unit != "um"
        || !(3..=128).contains(&spec.sections.len())
        || !(4..=128).contains(&landmark_count)
        || !valid_sections
        || spec
            .sections
            .windows(2)
            .any(|window| window[0].z_um >= window[1].z_um)
        || !section_ids.contains(spec.reference_section_id.as_str())
        || !spec.landmark_noise_standard_deviation_um.is_finite()
        || spec.landmark_noise_standard_deviation_um <= 0.0
        || !(100..=2_000).contains(&spec.posterior_draws)
        || !is_worker_timeout(spec.timeout_seconds)
    {
        return Err(
            "serial stacks require ordered unique physical sections, paired unique landmarks, a reference section, positive landmark noise, and bounded posterior draws"
                .into(),
        );
    }
    Ok(())
}

impl AdvancedValidationResult {
    pub fn validate_identity(
        &self,
        backend: &ValidationBackend,
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        if self.format != "marklab.advanced_3d_longitudinal_validation_suite"
            || self.version != 1
            || &self.backend != backend
            || self.request_sha256 != request_sha256
            || self.claim_status
                != ClaimStatus::Synthetic3dLongitudinalValidationWithExplicitRealGap
        {
            return Err("advanced 3-D validation result identity mismatch");
        }
        Ok(())
    }
}

impl CloneModelsResult {
    pub fn validate_identity(
        &self,
        backend: &ScipyBackend,
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        if self.format != "marklab.clone_phylogeography_and_niche_models"
            || self.version != 1
            || &self.backend != backend
            || self.request_sha256 != request_sha256
            || self.claim_status != ClaimStatus::ExperimentalSyntheticCloneModelsNoIdentifiedHistory
        {
            return Err("clone model result identity mismatch");
        }
        Ok(())
    }
}

impl DeformationBiologyResult {
    pub fn validate_identity(
        &self,
        backend: &ScipyBackend,
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        if self.format != "marklab.deformation_biology_model"
            || self.version != 1
            || &self.backend != backend
            || self.request_sha256 != request_sha256
            || self.claim_status != ClaimStatus::ExperimentalSyntheticDeformationBiologySeparation
        {
            return Err("deformation-biology result identity mismatch");
        }
        Ok(())
    }
}

impl AlphaComplexResult {
    pub fn validate_identity(
        &self,
        backend: &AlphaBackend,
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        if self.format != "marklab.alpha_complex_3d"
            || self.version != 1
            || &self.backend != backend
            || self.request_sha256 != request_sha256
            || self.claim_status != ClaimStatus::ExperimentalSyntheticExact3dAlphaComplex
        {
            return Err("3-D alpha result identity mismatch");
        }
        Ok(())
    }
}

impl SerialStackResult {
    pub fn validate_identity(
        &self,
        backend: &ScipyBackend,
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        if self.format != "marklab.bayesian_serial_section_stack"
            || self.version != 1
            || &self.backend != backend
            || self.request_sha256 != request_sha256
            || self.claim_status != ClaimStatus::ExperimentalSyntheticSerialStackReconstruction
        {
            return Err("serial stack result identity mismatch");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spatial_results_require_the_complete_typed_worker_shape() {
        let mut response = serde_json::json!({
            "format":"marklab.advanced_3d_longitudinal_validation_suite","version":1,
            "backend":{"name":"worker","gudhi_version":"gudhi","scipy_version":"scipy",
                "numpy_version":"numpy","python_version":"python","license":"GPLv3",
                "environment_lock_sha256":"lock","worker_sha256":"worker"},
            "request_sha256":"request","seed":1,
            "overall_status":"partial_synthetic_and_real_evidence_required",
            "entries":[
                {"validation_id":"numeric","status":"passed","metric":"error","value":0.0,"threshold":"<1e-12","evidence":"synthetic"},
                {"validation_id":"boolean","status":"passed","metric":"path_adjacency","value":true,"threshold":true,"evidence":"synthetic"},
                {"validation_id":"pair","status":"passed","metric":"effect_and_negative","value":[1.0,0.0],"threshold":"[1,0]","evidence":"synthetic"},
                {"validation_id":"unexecuted","status":"not_verified_unexecuted_control","metric":"gaps_um","value":null,"threshold":"explicit","evidence":"unexecuted"}
            ],
            "claim_status":"synthetic_3d_longitudinal_validation_with_explicit_real_gap"
        });
        let result: AdvancedValidationResult = serde_json::from_value(response.clone()).unwrap();
        result
            .validate_identity(&result.backend, "request")
            .unwrap();
        assert_eq!(serde_json::to_value(result).unwrap(), response);

        response["entries"][0]["value"] = serde_json::json!([1.0, 2.0, 3.0]);
        assert!(serde_json::from_value::<AdvancedValidationResult>(response.clone()).is_err());
        response["entries"][0]["value"] = serde_json::json!(0.0);
        response["entries"][0]["unrecognized_measurement"] = serde_json::json!(1);
        assert!(serde_json::from_value::<AdvancedValidationResult>(response).is_err());

        let interval = serde_json::json!({
            "birth_squared_alpha_um2":0.0,"death_squared_alpha_um2":null,"essential":true
        });
        let typed: AlphaInterval = serde_json::from_value(interval.clone()).unwrap();
        assert_eq!(serde_json::to_value(typed).unwrap(), interval);
        assert!(serde_json::from_str::<AlphaInterval>(
            r#"{"birth_squared_alpha_um2":0.0,"essential":true}"#
        )
        .is_err());
    }
}

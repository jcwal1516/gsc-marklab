use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use super::{
    validate_spd, CoordinateUnit, K3dCorrection, NormalizedPoint3D, Point3DInput, Spatial3dError,
};

const MAXIMUM_GRID_VOXELS: u64 = 1_000_000;
const MAXIMUM_OCCUPIED_VOXELS: usize = 250_000;
const MAXIMUM_PAIR_RADIUS_EVALUATIONS: u64 = 250_000_000;
const MAXIMUM_BOUNDARY_FACE_CHECKS: u64 = 250_000_000;
const MAXIMUM_TRANSLATION_VOXEL_PAIR_CHECKS: u64 = 250_000_000;
const MAXIMUM_MEMORY_MIB: usize = 4_096;

/// A finite physical voxel-union observation window.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoxelWindow3dInput {
    /// Physical lower corner before unit normalization.
    pub origin: [f64; 3],
    /// Positive physical voxel side lengths before unit normalization.
    pub voxel_size: [f64; 3],
    /// Grid cell counts in X, Y, and Z order.
    pub dimensions: [u32; 3],
    /// Unique occupied voxel indices in X, Y, and Z order.
    pub occupied_voxels: Vec<[u32; 3]>,
    /// Caller ceiling for the complete finite grid, including empty cells.
    pub maximum_grid_voxels: u64,
}

/// Bounded homogeneous K/L request over an exact physical voxel union.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoxelWindowK3dSpec {
    /// Unique points in the declared source unit.
    pub points: Vec<Point3DInput>,
    /// Exact segmented observation window.
    pub window: VoxelWindow3dInput,
    /// Unit shared by points and window geometry.
    pub coordinate_unit: CoordinateUnit,
    /// Optional symmetric positive-definite dimensionless metric.
    pub anisotropy_matrix: Option<[[f64; 3]; 3]>,
    /// Strictly increasing nonnegative physical radii in micrometres.
    pub radii_um: Vec<f64>,
    /// Named 3-D edge correction.
    pub correction: K3dCorrection,
    /// Caller ceiling for retained unordered point pairs.
    pub maximum_unordered_pairs: u64,
    /// Caller ceiling for point-to-exposed-face distance checks.
    pub maximum_boundary_face_checks: u64,
    /// Caller ceiling for exact voxel-box overlap checks.
    pub maximum_translation_voxel_pair_checks: u64,
    /// Caller retained-memory budget in mebibytes.
    pub memory_budget_mib: usize,
}

/// Canonical finite voxel-window geometry retained with a result.
#[derive(Clone, Debug, Serialize)]
pub struct VoxelWindow3dSummary {
    /// Fixed representation identity.
    pub representation: &'static str,
    /// Normalized physical lower corner.
    pub origin_um: [f64; 3],
    /// Normalized physical voxel side lengths.
    pub voxel_size_um: [f64; 3],
    /// Grid dimensions in X, Y, and Z order.
    pub dimensions: [u32; 3],
    /// Number of occupied voxels.
    pub occupied_voxel_count: usize,
    /// Exact occupied volume.
    pub volume_um3: f64,
    /// Exact exposed-face surface area, including cavity faces.
    pub surface_area_um2: f64,
    /// Six-connected occupied components.
    pub connected_components: usize,
    /// Six-connected empty components not touching the finite grid boundary.
    pub cavities: usize,
    /// Number of exposed axis-aligned voxel faces.
    pub exposed_face_count: usize,
    /// SHA-256 over normalized geometry and canonical occupied indices.
    pub logical_digest: String,
}

impl<'de> Deserialize<'de> for VoxelWindow3dSummary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Owned {
            representation: String,
            origin_um: [f64; 3],
            voxel_size_um: [f64; 3],
            dimensions: [u32; 3],
            occupied_voxel_count: usize,
            volume_um3: f64,
            surface_area_um2: f64,
            connected_components: usize,
            cavities: usize,
            exposed_face_count: usize,
            logical_digest: String,
        }
        let owned = Owned::deserialize(deserializer)?;
        if owned.representation != "axis_aligned_physical_voxel_union" {
            return Err(serde::de::Error::custom(
                "unexpected voxel-window representation",
            ));
        }
        Ok(Self {
            representation: "axis_aligned_physical_voxel_union",
            origin_um: owned.origin_um,
            voxel_size_um: owned.voxel_size_um,
            dimensions: owned.dimensions,
            occupied_voxel_count: owned.occupied_voxel_count,
            volume_um3: owned.volume_um3,
            surface_area_um2: owned.surface_area_um2,
            connected_components: owned.connected_components,
            cavities: owned.cavities,
            exposed_face_count: owned.exposed_face_count,
            logical_digest: owned.logical_digest,
        })
    }
}

/// One homogeneous K/L radius over the voxel-union window.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VoxelWindowK3dCurvePoint {
    /// Physical radius in micrometres.
    pub radius_um: f64,
    /// Homogeneous 3-D K estimate.
    pub k_um3: f64,
    /// Volume-equivalent 3-D L transform.
    pub l3_um: f64,
    /// Directed eligible pair contributions.
    pub eligible_directed_pairs: u64,
    /// Border-eligible reference points, when applicable.
    pub border_reference_points: Option<usize>,
    /// Sum of exact translation overlaps for contributing unordered pairs.
    pub translation_overlap_sum_um3: Option<f64>,
}

/// Typed homogeneous K/L result over a finite physical voxel union.
#[derive(Clone, Debug, Serialize)]
pub struct VoxelWindowK3dResult {
    /// Result identity.
    pub format: &'static str,
    /// Result schema version.
    pub version: u32,
    /// Spatial dimension.
    pub dimension: u32,
    /// Normalized coordinate unit.
    pub coordinate_unit: &'static str,
    /// Metric identity.
    pub metric: &'static str,
    /// Optional caller-supplied anisotropy matrix.
    pub anisotropy_matrix: Option<[[f64; 3]; 3]>,
    /// Edge correction identity.
    pub correction: K3dCorrection,
    /// Compiled exact window summary.
    pub window: VoxelWindow3dSummary,
    /// Normalized points in canonical input order.
    pub normalized_points: Vec<NormalizedPoint3D>,
    /// Radius-indexed K/L curve.
    pub curve: Vec<VoxelWindowK3dCurvePoint>,
    /// Retained unordered point pairs.
    pub unordered_pairs_visited: u64,
    /// Pair-radius eligibility evaluations.
    pub pair_radius_evaluations: u64,
    /// Point-to-exposed-face distance checks.
    pub boundary_face_checks: u64,
    /// Exact voxel-box translation-overlap checks.
    pub translation_voxel_pair_checks: u64,
    /// Conservative retained-memory estimate.
    pub retained_memory_bytes: usize,
    /// Caller memory budget in bytes.
    pub memory_budget_bytes: usize,
    /// Statistical unit.
    pub statistical_unit: &'static str,
    /// Conditioned null interpretation.
    pub null_model: &'static str,
    /// Finite-result behavior.
    pub finite_result_policy: &'static str,
    /// Claim ceiling.
    pub claim_status: &'static str,
}

impl<'de> Deserialize<'de> for VoxelWindowK3dResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Owned {
            format: String,
            version: u32,
            dimension: u32,
            coordinate_unit: String,
            metric: String,
            anisotropy_matrix: Option<[[f64; 3]; 3]>,
            correction: K3dCorrection,
            window: VoxelWindow3dSummary,
            normalized_points: Vec<NormalizedPoint3D>,
            curve: Vec<VoxelWindowK3dCurvePoint>,
            unordered_pairs_visited: u64,
            pair_radius_evaluations: u64,
            boundary_face_checks: u64,
            translation_voxel_pair_checks: u64,
            retained_memory_bytes: usize,
            memory_budget_bytes: usize,
            statistical_unit: String,
            null_model: String,
            finite_result_policy: String,
            claim_status: String,
        }
        let owned = Owned::deserialize(deserializer)?;
        let metric = match owned.metric.as_str() {
            "euclidean_physical_um" => "euclidean_physical_um",
            "anisotropic_mahalanobis_physical_um" => "anisotropic_mahalanobis_physical_um",
            _ => return Err(serde::de::Error::custom("unexpected voxel-window metric")),
        };
        if owned.format != "marklab.voxel_window_homogeneous_k3d"
            || owned.coordinate_unit != "micrometer"
            || owned.statistical_unit != "one_3d_point_pattern_observed_in_one_segmented_volume"
            || owned.null_model
                != "homogeneous_process_conditioned_on_exact_voxel_union_observation_window"
            || owned.finite_result_policy
                != "reject_non_finite_or_undefined_radius_no_infinity_persisted"
            || owned.claim_status != "descriptive_voxel_window_3d_point_process_only"
        {
            return Err(serde::de::Error::custom(
                "unexpected voxel-window K result identity",
            ));
        }
        Ok(Self {
            format: "marklab.voxel_window_homogeneous_k3d",
            version: owned.version,
            dimension: owned.dimension,
            coordinate_unit: "micrometer",
            metric,
            anisotropy_matrix: owned.anisotropy_matrix,
            correction: owned.correction,
            window: owned.window,
            normalized_points: owned.normalized_points,
            curve: owned.curve,
            unordered_pairs_visited: owned.unordered_pairs_visited,
            pair_radius_evaluations: owned.pair_radius_evaluations,
            boundary_face_checks: owned.boundary_face_checks,
            translation_voxel_pair_checks: owned.translation_voxel_pair_checks,
            retained_memory_bytes: owned.retained_memory_bytes,
            memory_budget_bytes: owned.memory_budget_bytes,
            statistical_unit: "one_3d_point_pattern_observed_in_one_segmented_volume",
            null_model: "homogeneous_process_conditioned_on_exact_voxel_union_observation_window",
            finite_result_policy: "reject_non_finite_or_undefined_radius_no_infinity_persisted",
            claim_status: "descriptive_voxel_window_3d_point_process_only",
        })
    }
}

impl VoxelWindowK3dResult {
    /// Validate a restored result against its exact typed request without recomputing overlaps.
    pub fn validate_for_spec(&self, spec: &VoxelWindowK3dSpec) -> Result<(), Spatial3dError> {
        let scale = spec.coordinate_unit.micrometer_scale();
        let mut occupied = spec.window.occupied_voxels.clone();
        occupied.sort_unstable();
        let origin_um = spec.window.origin.map(|value| value * scale);
        let voxel_size_um = spec.window.voxel_size.map(|value| value * scale);
        let expected_digest =
            logical_digest(origin_um, voxel_size_um, spec.window.dimensions, &occupied);
        let expected_pairs = unordered_pair_count(spec.points.len())?;
        let expected_pair_radius_work = expected_pairs
            .checked_mul(spec.radii_um.len() as u64)
            .ok_or_else(|| {
                Spatial3dError::Resource("restored pair-radius work overflowed".into())
            })?;
        let points_match = self.normalized_points.len() == spec.points.len()
            && self
                .normalized_points
                .iter()
                .zip(&spec.points)
                .all(|(observed, expected)| {
                    observed.id == expected.id
                        && observed
                            .coordinates_um
                            .iter()
                            .zip(expected.coordinates.map(|value| value * scale))
                            .all(|(left, right)| left.to_bits() == right.to_bits())
                });
        let metric =
            spec.anisotropy_matrix
                .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        let contributing_pairs = pairs_within_maximum(
            &self.normalized_points,
            metric,
            *spec
                .radii_um
                .last()
                .ok_or_else(|| Spatial3dError::Invalid("restored request has no radii".into()))?,
        )?;
        let expected_translation_checks = if spec.correction == K3dCorrection::Translation {
            contributing_pairs
                .checked_mul(occupied.len() as u64)
                .and_then(|work| work.checked_mul(occupied.len() as u64))
                .ok_or_else(|| {
                    Spatial3dError::Resource("restored translation work overflowed".into())
                })?
        } else {
            0
        };
        let expected_boundary_checks = if spec.correction == K3dCorrection::Border {
            (spec.points.len() as u64)
                .checked_mul(self.window.exposed_face_count as u64)
                .ok_or_else(|| {
                    Spatial3dError::Resource("restored boundary work overflowed".into())
                })?
        } else {
            0
        };
        let finite_curve = self.curve.iter().all(|row| {
            [
                row.radius_um,
                row.k_um3,
                row.l3_um,
                row.translation_overlap_sum_um3.unwrap_or(0.0),
            ]
            .into_iter()
            .all(f64::is_finite)
                && row.k_um3 >= 0.0
        });
        let radii_match = self.curve.len() == spec.radii_um.len()
            && self
                .curve
                .iter()
                .zip(&spec.radii_um)
                .all(|(row, radius)| row.radius_um.to_bits() == radius.to_bits());
        if self.version != 1
            || self.dimension != 3
            || self.correction != spec.correction
            || self.anisotropy_matrix != spec.anisotropy_matrix
            || self.window.origin_um != origin_um
            || self.window.voxel_size_um != voxel_size_um
            || self.window.dimensions != spec.window.dimensions
            || self.window.occupied_voxel_count != occupied.len()
            || self.window.logical_digest != expected_digest
            || self.window.connected_components == 0
            || self.window.exposed_face_count == 0
            || !self.window.volume_um3.is_finite()
            || self.window.volume_um3 <= 0.0
            || !self.window.surface_area_um2.is_finite()
            || self.window.surface_area_um2 <= 0.0
            || !points_match
            || !radii_match
            || !finite_curve
            || self.unordered_pairs_visited != expected_pairs
            || self.pair_radius_evaluations != expected_pair_radius_work
            || self.translation_voxel_pair_checks != expected_translation_checks
            || self.boundary_face_checks != expected_boundary_checks
            || self.translation_voxel_pair_checks > spec.maximum_translation_voxel_pair_checks
            || self.boundary_face_checks > spec.maximum_boundary_face_checks
            || self.memory_budget_bytes != spec.memory_budget_mib * 1024 * 1024
            || self.retained_memory_bytes > self.memory_budget_bytes
        {
            return Err(Spatial3dError::Invalid(
                "restored voxel-window K result differs from its request identity or bounds".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Face {
    axis: usize,
    coordinate: f64,
    ranges: [[f64; 2]; 2],
}

struct CompiledWindow {
    origin_um: [f64; 3],
    voxel_size_um: [f64; 3],
    dimensions: [u32; 3],
    occupied: Vec<[u32; 3]>,
    occupied_grid: Vec<bool>,
    faces: Vec<Face>,
    summary: VoxelWindow3dSummary,
}

struct Pair {
    distance: f64,
    translation_overlap_um3: Option<f64>,
}

/// Compute bounded homogeneous 3-D K/L without replacing the exact tissue window by a cuboid.
pub fn voxel_window_k3d(spec: VoxelWindowK3dSpec) -> Result<VoxelWindowK3dResult, Spatial3dError> {
    validate_controls(&spec)?;
    let scale = spec.coordinate_unit.micrometer_scale();
    let mut window = compile_window(&spec.window, scale)?;
    let pair_count = unordered_pair_count(spec.points.len())?;
    if pair_count > spec.maximum_unordered_pairs {
        return Err(Spatial3dError::Resource(format!(
            "unordered point pairs {pair_count} exceed caller maximum {}",
            spec.maximum_unordered_pairs
        )));
    }
    let pair_radius_work = pair_count
        .checked_mul(spec.radii_um.len() as u64)
        .ok_or_else(|| Spatial3dError::Resource("pair-radius work overflowed".into()))?;
    if pair_radius_work > MAXIMUM_PAIR_RADIUS_EVALUATIONS {
        return Err(Spatial3dError::Resource(format!(
            "pair-radius evaluations {pair_radius_work} exceed built-in maximum"
        )));
    }
    let memory_budget_bytes = spec
        .memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| Spatial3dError::Resource("memory budget overflowed".into()))?;
    let retained_memory_bytes = retained_memory_estimate(
        window.occupied_grid.len(),
        window.occupied.len(),
        window.faces.len(),
        spec.points.len(),
        pair_count,
        spec.radii_um.len(),
    )?;
    if retained_memory_bytes > memory_budget_bytes {
        return Err(Spatial3dError::Resource(format!(
            "retained-memory estimate {retained_memory_bytes} exceeds caller budget {memory_budget_bytes}"
        )));
    }

    let normalized_points = normalize_points(&spec.points, scale, &window)?;
    let metric =
        spec.anisotropy_matrix
            .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    validate_spd(metric)?;
    let maximum_radius = *spec.radii_um.last().expect("validated nonempty radii");
    let contributing_pair_count = pairs_within_maximum(&normalized_points, metric, maximum_radius)?;
    let translation_checks = if spec.correction == K3dCorrection::Translation {
        contributing_pair_count
            .checked_mul(window.occupied.len() as u64)
            .and_then(|work| work.checked_mul(window.occupied.len() as u64))
            .ok_or_else(|| Spatial3dError::Resource("translation overlap work overflowed".into()))?
    } else {
        0
    };
    if translation_checks > spec.maximum_translation_voxel_pair_checks
        || translation_checks > MAXIMUM_TRANSLATION_VOXEL_PAIR_CHECKS
    {
        return Err(Spatial3dError::Resource(format!(
            "translation voxel-pair checks {translation_checks} exceed caller or built-in maximum"
        )));
    }
    let boundary_checks = if spec.correction == K3dCorrection::Border {
        (normalized_points.len() as u64)
            .checked_mul(window.faces.len() as u64)
            .ok_or_else(|| Spatial3dError::Resource("boundary-face work overflowed".into()))?
    } else {
        0
    };
    if boundary_checks > spec.maximum_boundary_face_checks
        || boundary_checks > MAXIMUM_BOUNDARY_FACE_CHECKS
    {
        return Err(Spatial3dError::Resource(format!(
            "boundary-face checks {boundary_checks} exceed caller or built-in maximum"
        )));
    }

    let pairs = build_pairs(
        &normalized_points,
        metric,
        maximum_radius,
        &window,
        spec.correction,
    )?;
    let boundary_distances = if spec.correction == K3dCorrection::Border {
        normalized_points
            .iter()
            .map(|point| boundary_distance(point.coordinates_um, &window.faces))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let volume = window.summary.volume_um3;
    let point_count = normalized_points.len();
    let mut curve = Vec::with_capacity(spec.radii_um.len());
    for radius in &spec.radii_um {
        let (weighted_sum, eligible, border_references, overlap_sum) = match spec.correction {
            K3dCorrection::None => {
                let unordered = pairs.iter().filter(|pair| pair.distance <= *radius).count() as u64;
                (2.0 * unordered as f64, 2 * unordered, None, None)
            }
            K3dCorrection::Translation => {
                let mut sum = 0.0;
                let mut overlap_sum = 0.0;
                let mut eligible = 0_u64;
                for pair in pairs.iter().filter(|pair| pair.distance <= *radius) {
                    let overlap = pair
                        .translation_overlap_um3
                        .expect("translation pairs retain overlap");
                    if overlap <= 0.0 || !overlap.is_finite() {
                        return Err(Spatial3dError::Numerical(
                            "translation overlap is zero or non-finite for an eligible pair".into(),
                        ));
                    }
                    sum += 2.0 * volume / overlap;
                    overlap_sum += overlap;
                    eligible += 2;
                }
                (sum, eligible, None, Some(overlap_sum))
            }
            K3dCorrection::Border => {
                let references = boundary_distances
                    .iter()
                    .filter(|distance| **distance >= *radius)
                    .count();
                if references == 0 {
                    return Err(Spatial3dError::Invalid(format!(
                        "border correction has no reference point at radius {radius}"
                    )));
                }
                let mut eligible = 0_u64;
                for left in 0..point_count {
                    for right in (left + 1)..point_count {
                        let pair_index = pair_index(left, right, point_count);
                        if pairs[pair_index].distance <= *radius {
                            eligible += u64::from(boundary_distances[left] >= *radius);
                            eligible += u64::from(boundary_distances[right] >= *radius);
                        }
                    }
                }
                (eligible as f64, eligible, Some(references), None)
            }
        };
        let denominator = match border_references {
            Some(references) => (references * (point_count - 1)) as f64,
            None => (point_count * (point_count - 1)) as f64,
        };
        let k = volume * weighted_sum / denominator;
        let l3 = (3.0 * k / (4.0 * std::f64::consts::PI)).cbrt();
        if !k.is_finite() || !l3.is_finite() {
            return Err(Spatial3dError::Numerical(
                "voxel-window K/L result is not finite".into(),
            ));
        }
        curve.push(VoxelWindowK3dCurvePoint {
            radius_um: *radius,
            k_um3: k,
            l3_um: l3,
            eligible_directed_pairs: eligible,
            border_reference_points: border_references,
            translation_overlap_sum_um3: overlap_sum,
        });
    }

    window.occupied_grid.clear();
    Ok(VoxelWindowK3dResult {
        format: "marklab.voxel_window_homogeneous_k3d",
        version: 1,
        dimension: 3,
        coordinate_unit: "micrometer",
        metric: if spec.anisotropy_matrix.is_some() {
            "anisotropic_mahalanobis_physical_um"
        } else {
            "euclidean_physical_um"
        },
        anisotropy_matrix: spec.anisotropy_matrix,
        correction: spec.correction,
        window: window.summary,
        normalized_points,
        curve,
        unordered_pairs_visited: pair_count,
        pair_radius_evaluations: pair_radius_work,
        boundary_face_checks: boundary_checks,
        translation_voxel_pair_checks: translation_checks,
        retained_memory_bytes,
        memory_budget_bytes,
        statistical_unit: "one_3d_point_pattern_observed_in_one_segmented_volume",
        null_model: "homogeneous_process_conditioned_on_exact_voxel_union_observation_window",
        finite_result_policy: "reject_non_finite_or_undefined_radius_no_infinity_persisted",
        claim_status: "descriptive_voxel_window_3d_point_process_only",
    })
}

fn validate_controls(spec: &VoxelWindowK3dSpec) -> Result<(), Spatial3dError> {
    if spec.radii_um.is_empty()
        || spec
            .radii_um
            .iter()
            .any(|radius| !radius.is_finite() || *radius < 0.0)
        || spec.radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        || spec.maximum_unordered_pairs == 0
        || spec.maximum_boundary_face_checks == 0
        || spec.maximum_translation_voxel_pair_checks == 0
        || spec.memory_budget_mib == 0
        || spec.memory_budget_mib > MAXIMUM_MEMORY_MIB
    {
        return Err(Spatial3dError::Invalid(
            "voxel-window K requires increasing finite radii and positive bounded work/memory controls"
                .into(),
        ));
    }
    Ok(())
}

fn compile_window(
    input: &VoxelWindow3dInput,
    scale: f64,
) -> Result<CompiledWindow, Spatial3dError> {
    if input.origin.iter().any(|value| !value.is_finite())
        || input
            .voxel_size
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        || input.dimensions.contains(&0)
        || input.occupied_voxels.is_empty()
        || input.occupied_voxels.len() > MAXIMUM_OCCUPIED_VOXELS
        || input.maximum_grid_voxels == 0
        || input.maximum_grid_voxels > MAXIMUM_GRID_VOXELS
    {
        return Err(Spatial3dError::Invalid(
            "voxel window requires finite origin, positive voxel size/dimensions, occupied voxels, and bounded grid controls"
                .into(),
        ));
    }
    let grid_cells = input
        .dimensions
        .iter()
        .try_fold(1_u64, |product, value| {
            product.checked_mul(u64::from(*value))
        })
        .ok_or_else(|| Spatial3dError::Resource("voxel grid size overflowed".into()))?;
    if grid_cells > input.maximum_grid_voxels || grid_cells > MAXIMUM_GRID_VOXELS {
        return Err(Spatial3dError::Resource(format!(
            "voxel grid cells {grid_cells} exceed caller or built-in maximum"
        )));
    }
    let grid_len = usize::try_from(grid_cells)
        .map_err(|_| Spatial3dError::Resource("voxel grid does not fit memory indexing".into()))?;
    let origin_um = input.origin.map(|value| value * scale);
    let voxel_size_um = input.voxel_size.map(|value| value * scale);
    let mut occupied = input.occupied_voxels.clone();
    occupied.sort_unstable();
    if occupied.windows(2).any(|pair| pair[0] == pair[1])
        || occupied.iter().any(|index| {
            index
                .iter()
                .zip(input.dimensions)
                .any(|(value, dimension)| *value >= dimension)
        })
    {
        return Err(Spatial3dError::Invalid(
            "occupied voxel indices must be unique and within the declared grid".into(),
        ));
    }
    let mut occupied_grid = vec![false; grid_len];
    for coordinate in &occupied {
        occupied_grid[linear_index(*coordinate, input.dimensions)] = true;
    }
    let connected_components = component_count(&occupied_grid, input.dimensions, true).0;
    let (empty_components, exterior_empty_components) =
        component_count(&occupied_grid, input.dimensions, false);
    let cavities = empty_components.saturating_sub(exterior_empty_components);
    let faces = exposed_faces(
        &occupied,
        &occupied_grid,
        input.dimensions,
        origin_um,
        voxel_size_um,
    );
    let voxel_volume = voxel_size_um.iter().product::<f64>();
    let volume_um3 = voxel_volume * occupied.len() as f64;
    let surface_area_um2 = faces
        .iter()
        .map(|face| {
            let axes = other_axes(face.axis);
            voxel_size_um[axes[0]] * voxel_size_um[axes[1]]
        })
        .sum::<f64>();
    if !volume_um3.is_finite() || !surface_area_um2.is_finite() {
        return Err(Spatial3dError::Numerical(
            "voxel window measures are not finite".into(),
        ));
    }
    let logical_digest = logical_digest(origin_um, voxel_size_um, input.dimensions, &occupied);
    Ok(CompiledWindow {
        origin_um,
        voxel_size_um,
        dimensions: input.dimensions,
        occupied,
        occupied_grid,
        summary: VoxelWindow3dSummary {
            representation: "axis_aligned_physical_voxel_union",
            origin_um,
            voxel_size_um,
            dimensions: input.dimensions,
            occupied_voxel_count: input.occupied_voxels.len(),
            volume_um3,
            surface_area_um2,
            connected_components,
            cavities,
            exposed_face_count: faces.len(),
            logical_digest,
        },
        faces,
    })
}

fn normalize_points(
    input: &[Point3DInput],
    scale: f64,
    window: &CompiledWindow,
) -> Result<Vec<NormalizedPoint3D>, Spatial3dError> {
    if input.len() < 2 {
        return Err(Spatial3dError::Invalid(
            "voxel-window K requires at least two points".into(),
        ));
    }
    let mut ids = HashSet::with_capacity(input.len());
    input
        .iter()
        .map(|point| {
            if point.id.trim().is_empty() || !ids.insert(point.id.as_str()) {
                return Err(Spatial3dError::Invalid(
                    "point IDs must be unique and nonempty".into(),
                ));
            }
            let coordinates_um = point.coordinates.map(|value| value * scale);
            if coordinates_um.iter().any(|value| !value.is_finite())
                || !contains(window, coordinates_um)
            {
                return Err(Spatial3dError::Invalid(format!(
                    "point {} lies outside the occupied voxel union",
                    point.id
                )));
            }
            Ok(NormalizedPoint3D {
                id: point.id.clone(),
                coordinates_um,
            })
        })
        .collect()
}

fn contains(window: &CompiledWindow, point: [f64; 3]) -> bool {
    let mut index = [0_u32; 3];
    for axis in 0..3 {
        let relative = (point[axis] - window.origin_um[axis]) / window.voxel_size_um[axis];
        if !relative.is_finite() || relative < 0.0 || relative >= f64::from(window.dimensions[axis])
        {
            return false;
        }
        index[axis] = relative.floor() as u32;
    }
    window.occupied_grid[linear_index(index, window.dimensions)]
}

fn build_pairs(
    points: &[NormalizedPoint3D],
    metric: [[f64; 3]; 3],
    maximum_radius: f64,
    window: &CompiledWindow,
    correction: K3dCorrection,
) -> Result<Vec<Pair>, Spatial3dError> {
    let mut pairs = Vec::with_capacity(points.len() * (points.len() - 1) / 2);
    for left in 0..points.len() {
        for right in (left + 1)..points.len() {
            let displacement = std::array::from_fn(|axis| {
                points[right].coordinates_um[axis] - points[left].coordinates_um[axis]
            });
            let distance = metric_distance(displacement, metric)?;
            let translation_overlap_um3 =
                if correction == K3dCorrection::Translation && distance <= maximum_radius {
                    Some(translation_overlap(window, displacement))
                } else {
                    None
                };
            pairs.push(Pair {
                distance,
                translation_overlap_um3,
            });
        }
    }
    Ok(pairs)
}

fn pairs_within_maximum(
    points: &[NormalizedPoint3D],
    metric: [[f64; 3]; 3],
    maximum_radius: f64,
) -> Result<u64, Spatial3dError> {
    let mut count = 0_u64;
    for left in 0..points.len() {
        for right in (left + 1)..points.len() {
            let displacement = std::array::from_fn(|axis| {
                points[right].coordinates_um[axis] - points[left].coordinates_um[axis]
            });
            count += u64::from(metric_distance(displacement, metric)? <= maximum_radius);
        }
    }
    Ok(count)
}

fn metric_distance(displacement: [f64; 3], metric: [[f64; 3]; 3]) -> Result<f64, Spatial3dError> {
    let transformed: [f64; 3] = std::array::from_fn(|row| {
        (0..3)
            .map(|column| metric[row][column] * displacement[column])
            .sum()
    });
    let squared = displacement
        .iter()
        .zip(transformed)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    if !squared.is_finite() || squared < 0.0 {
        return Err(Spatial3dError::Numerical(
            "anisotropic squared distance is negative or non-finite".into(),
        ));
    }
    Ok(squared.sqrt())
}

fn translation_overlap(window: &CompiledWindow, displacement: [f64; 3]) -> f64 {
    let mut total = 0.0;
    for left in &window.occupied {
        let left_min: [f64; 3] = std::array::from_fn(|axis| {
            window.origin_um[axis] + f64::from(left[axis]) * window.voxel_size_um[axis]
        });
        for right in &window.occupied {
            let right_min: [f64; 3] = std::array::from_fn(|axis| {
                window.origin_um[axis]
                    + f64::from(right[axis]) * window.voxel_size_um[axis]
                    + displacement[axis]
            });
            total += (0..3)
                .map(|axis| {
                    (left_min[axis] + window.voxel_size_um[axis])
                        .min(right_min[axis] + window.voxel_size_um[axis])
                        - left_min[axis].max(right_min[axis])
                })
                .map(|length| length.max(0.0))
                .product::<f64>();
        }
    }
    total
}

fn boundary_distance(point: [f64; 3], faces: &[Face]) -> Result<f64, Spatial3dError> {
    let minimum = faces
        .iter()
        .map(|face| {
            let axes = other_axes(face.axis);
            let normal = point[face.axis] - face.coordinate;
            let first = interval_distance(point[axes[0]], face.ranges[0]);
            let second = interval_distance(point[axes[1]], face.ranges[1]);
            (normal * normal + first * first + second * second).sqrt()
        })
        .fold(f64::INFINITY, f64::min);
    if !minimum.is_finite() {
        return Err(Spatial3dError::Numerical(
            "point-to-voxel-boundary distance is not finite".into(),
        ));
    }
    Ok(minimum)
}

fn interval_distance(value: f64, interval: [f64; 2]) -> f64 {
    if value < interval[0] {
        interval[0] - value
    } else if value > interval[1] {
        value - interval[1]
    } else {
        0.0
    }
}

fn exposed_faces(
    occupied: &[[u32; 3]],
    grid: &[bool],
    dimensions: [u32; 3],
    origin: [f64; 3],
    voxel_size: [f64; 3],
) -> Vec<Face> {
    let mut faces = Vec::new();
    for voxel in occupied {
        for axis in 0..3 {
            for positive in [false, true] {
                let neighbor = neighbor(*voxel, axis, positive, dimensions);
                if neighbor.is_some_and(|index| grid[linear_index(index, dimensions)]) {
                    continue;
                }
                let axes = other_axes(axis);
                let coordinate = origin[axis]
                    + (f64::from(voxel[axis]) + f64::from(positive)) * voxel_size[axis];
                faces.push(Face {
                    axis,
                    coordinate,
                    ranges: axes.map(|other| {
                        let minimum = origin[other] + f64::from(voxel[other]) * voxel_size[other];
                        [minimum, minimum + voxel_size[other]]
                    }),
                });
            }
        }
    }
    faces
}

fn component_count(grid: &[bool], dimensions: [u32; 3], occupied: bool) -> (usize, usize) {
    let mut visited = vec![false; grid.len()];
    let mut components = 0_usize;
    let mut boundary_components = 0_usize;
    for linear in 0..grid.len() {
        if visited[linear] || grid[linear] != occupied {
            continue;
        }
        components += 1;
        let mut touches_boundary = false;
        let mut queue = VecDeque::from([coordinate_from_linear(linear, dimensions)]);
        visited[linear] = true;
        while let Some(coordinate) = queue.pop_front() {
            touches_boundary |= coordinate
                .iter()
                .zip(dimensions)
                .any(|(value, dimension)| *value == 0 || *value + 1 == dimension);
            for axis in 0..3 {
                for positive in [false, true] {
                    let Some(next) = neighbor(coordinate, axis, positive, dimensions) else {
                        continue;
                    };
                    let next_linear = linear_index(next, dimensions);
                    if !visited[next_linear] && grid[next_linear] == occupied {
                        visited[next_linear] = true;
                        queue.push_back(next);
                    }
                }
            }
        }
        boundary_components += usize::from(touches_boundary);
    }
    (components, boundary_components)
}

fn neighbor(
    mut coordinate: [u32; 3],
    axis: usize,
    positive: bool,
    dimensions: [u32; 3],
) -> Option<[u32; 3]> {
    if positive {
        if coordinate[axis] + 1 >= dimensions[axis] {
            return None;
        }
        coordinate[axis] += 1;
    } else {
        if coordinate[axis] == 0 {
            return None;
        }
        coordinate[axis] -= 1;
    }
    Some(coordinate)
}

fn linear_index(coordinate: [u32; 3], dimensions: [u32; 3]) -> usize {
    coordinate[0] as usize
        + dimensions[0] as usize
            * (coordinate[1] as usize + dimensions[1] as usize * coordinate[2] as usize)
}

fn coordinate_from_linear(linear: usize, dimensions: [u32; 3]) -> [u32; 3] {
    let x_size = dimensions[0] as usize;
    let y_size = dimensions[1] as usize;
    let x = linear % x_size;
    let remainder = linear / x_size;
    let y = remainder % y_size;
    let z = remainder / y_size;
    [x as u32, y as u32, z as u32]
}

fn other_axes(axis: usize) -> [usize; 2] {
    match axis {
        0 => [1, 2],
        1 => [0, 2],
        2 => [0, 1],
        _ => unreachable!("3-D axis"),
    }
}

fn unordered_pair_count(points: usize) -> Result<u64, Spatial3dError> {
    let points = u64::try_from(points)
        .map_err(|_| Spatial3dError::Resource("point count overflowed".into()))?;
    points
        .checked_mul(points.saturating_sub(1))
        .map(|pairs| pairs / 2)
        .ok_or_else(|| Spatial3dError::Resource("unordered point count overflowed".into()))
}

fn pair_index(left: usize, right: usize, point_count: usize) -> usize {
    left * (2 * point_count - left - 1) / 2 + (right - left - 1)
}

fn retained_memory_estimate(
    grid_cells: usize,
    occupied: usize,
    faces: usize,
    points: usize,
    pairs: u64,
    radii: usize,
) -> Result<usize, Spatial3dError> {
    let pairs = usize::try_from(pairs)
        .map_err(|_| Spatial3dError::Resource("pair memory count overflowed".into()))?;
    grid_cells
        .checked_mul(24)
        .and_then(|bytes| bytes.checked_add(occupied.checked_mul(32)?))
        .and_then(|bytes| bytes.checked_add(faces.checked_mul(64)?))
        .and_then(|bytes| bytes.checked_add(points.checked_mul(160)?))
        .and_then(|bytes| bytes.checked_add(pairs.checked_mul(64)?))
        .and_then(|bytes| bytes.checked_add(radii.checked_mul(96)?))
        .ok_or_else(|| Spatial3dError::Resource("retained-memory estimate overflowed".into()))
}

fn logical_digest(
    origin: [f64; 3],
    voxel_size: [f64; 3],
    dimensions: [u32; 3],
    occupied: &[[u32; 3]],
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"marklab-physical-voxel-window-v1\0");
    for value in origin.into_iter().chain(voxel_size) {
        digest.update(value.to_bits().to_le_bytes());
    }
    for value in dimensions {
        digest.update(value.to_le_bytes());
    }
    for voxel in occupied {
        for value in voxel {
            digest.update(value.to_le_bytes());
        }
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l_spec(correction: K3dCorrection) -> VoxelWindowK3dSpec {
        VoxelWindowK3dSpec {
            points: vec![
                Point3DInput {
                    id: "a".into(),
                    coordinates: [0.5, 0.5, 0.5],
                },
                Point3DInput {
                    id: "b".into(),
                    coordinates: [1.5, 0.5, 0.5],
                },
            ],
            window: VoxelWindow3dInput {
                origin: [0.0; 3],
                voxel_size: [1.0; 3],
                dimensions: [2, 2, 1],
                occupied_voxels: vec![[0, 0, 0], [1, 0, 0], [0, 1, 0]],
                maximum_grid_voxels: 4,
            },
            coordinate_unit: CoordinateUnit::Micrometer,
            anisotropy_matrix: None,
            radii_um: vec![1.0],
            correction,
            maximum_unordered_pairs: 1,
            maximum_boundary_face_checks: 28,
            maximum_translation_voxel_pair_checks: 9,
            memory_budget_mib: 1,
        }
    }

    #[test]
    fn border_work_is_checked_before_distance_evaluation() {
        let mut spec = l_spec(K3dCorrection::Border);
        spec.maximum_boundary_face_checks = 27;
        let error = voxel_window_k3d(spec).unwrap_err();
        assert!(error.to_string().contains("boundary-face checks 28"));
    }

    #[test]
    fn translation_work_is_checked_exactly() {
        let mut spec = l_spec(K3dCorrection::Translation);
        spec.maximum_translation_voxel_pair_checks = 8;
        let error = voxel_window_k3d(spec).unwrap_err();
        assert!(error.to_string().contains("voxel-pair checks 9"));
    }
}

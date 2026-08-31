#![forbid(unsafe_code)]
//! Dimension-aware bounded three-dimensional spatial statistics.

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

mod graph;
mod weighted;

pub use graph::{
    build_spatial_graph3d, DistanceBasis3D, Graph3dEdge, Graph3dPoint, GraphRule3D, GraphWeight3D,
    SparseGraphOperator3D, SpatialGraph3dResult, SpatialGraph3dSpec,
};

pub use weighted::{
    directed_cross_k3d, inhomogeneous_k3d, CrossK3dCurvePoint, DirectedCrossK3dResult,
    DirectedCrossK3dSpec, InhomogeneousK3dResult, InhomogeneousK3dSpec, NormalizedWeightedPoint3D,
    WeightedK3dCurvePoint, WeightedPoint3DInput,
};

const MAXIMUM_PAIR_RADIUS_EVALUATIONS: u64 = 250_000_000;
const MAXIMUM_RETAINED_UNORDERED_PAIRS: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateUnit {
    Nanometer,
    Micrometer,
    Millimeter,
}

impl CoordinateUnit {
    pub(crate) fn micrometer_scale(self) -> f64 {
        match self {
            Self::Nanometer => 0.001,
            Self::Micrometer => 1.0,
            Self::Millimeter => 1_000.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point3DInput {
    pub id: String,
    pub coordinates: [f64; 3],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CuboidWindowInput {
    pub minimum: [f64; 3],
    pub maximum: [f64; 3],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum K3dCorrection {
    None,
    Border,
    Translation,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomogeneousK3dSpec {
    pub points: Vec<Point3DInput>,
    pub window: CuboidWindowInput,
    pub coordinate_unit: CoordinateUnit,
    pub voxel_spacing: [f64; 3],
    pub anisotropy_matrix: Option<[[f64; 3]; 3]>,
    pub radii_um: Vec<f64>,
    pub correction: K3dCorrection,
    pub maximum_unordered_pairs: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedPoint3D {
    pub id: String,
    pub coordinates_um: [f64; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct CompiledCuboid3D {
    pub representation: &'static str,
    pub minimum_um: [f64; 3],
    pub maximum_um: [f64; 3],
    pub side_lengths_um: [f64; 3],
    pub volume_um3: f64,
    pub surface_area_um2: f64,
    pub components: u32,
    pub cavities: u32,
}

impl<'de> Deserialize<'de> for CompiledCuboid3D {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct OwnedWindow {
            representation: String,
            minimum_um: [f64; 3],
            maximum_um: [f64; 3],
            side_lengths_um: [f64; 3],
            volume_um3: f64,
            surface_area_um2: f64,
            components: u32,
            cavities: u32,
        }

        let owned = OwnedWindow::deserialize(deserializer)?;
        if owned.representation != "axis_aligned_cuboid" {
            return Err(serde::de::Error::custom(
                "unexpected compiled 3-D window representation",
            ));
        }
        Ok(Self {
            representation: "axis_aligned_cuboid",
            minimum_um: owned.minimum_um,
            maximum_um: owned.maximum_um,
            side_lengths_um: owned.side_lengths_um,
            volume_um3: owned.volume_um3,
            surface_area_um2: owned.surface_area_um2,
            components: owned.components,
            cavities: owned.cavities,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct K3dCurvePoint {
    pub radius_um: f64,
    pub k_um3: f64,
    pub l3_um: f64,
    pub eligible_directed_pairs: u64,
    pub border_reference_points: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HomogeneousK3dResult {
    pub format: &'static str,
    pub version: u32,
    pub dimension: u32,
    pub coordinate_unit: &'static str,
    pub normalized_voxel_spacing_um: [f64; 3],
    pub metric: &'static str,
    pub anisotropy_matrix: Option<[[f64; 3]; 3]>,
    pub correction: K3dCorrection,
    pub window: CompiledCuboid3D,
    pub normalized_points: Vec<NormalizedPoint3D>,
    pub curve: Vec<K3dCurvePoint>,
    pub unordered_pairs_visited: u64,
    pub pair_radius_evaluations: u64,
    pub claim_status: &'static str,
}

impl<'de> Deserialize<'de> for HomogeneousK3dResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct OwnedResult {
            format: String,
            version: u32,
            dimension: u32,
            coordinate_unit: String,
            normalized_voxel_spacing_um: [f64; 3],
            metric: String,
            anisotropy_matrix: Option<[[f64; 3]; 3]>,
            correction: K3dCorrection,
            window: CompiledCuboid3D,
            normalized_points: Vec<NormalizedPoint3D>,
            curve: Vec<K3dCurvePoint>,
            unordered_pairs_visited: u64,
            pair_radius_evaluations: u64,
            claim_status: String,
        }

        let owned = OwnedResult::deserialize(deserializer)?;
        let metric = match owned.metric.as_str() {
            "euclidean_physical_um" => "euclidean_physical_um",
            "anisotropic_mahalanobis_physical_um" => "anisotropic_mahalanobis_physical_um",
            _ => return Err(serde::de::Error::custom("unexpected 3-D metric identity")),
        };
        if owned.format != "marklab.homogeneous_k3d"
            || owned.coordinate_unit != "micrometer"
            || owned.claim_status != "homogeneous_cuboid_3d_descriptive_only"
        {
            return Err(serde::de::Error::custom(
                "unexpected homogeneous 3-D result identity",
            ));
        }
        Ok(Self {
            format: "marklab.homogeneous_k3d",
            version: owned.version,
            dimension: owned.dimension,
            coordinate_unit: "micrometer",
            normalized_voxel_spacing_um: owned.normalized_voxel_spacing_um,
            metric,
            anisotropy_matrix: owned.anisotropy_matrix,
            correction: owned.correction,
            window: owned.window,
            normalized_points: owned.normalized_points,
            curve: owned.curve,
            unordered_pairs_visited: owned.unordered_pairs_visited,
            pair_radius_evaluations: owned.pair_radius_evaluations,
            claim_status: "homogeneous_cuboid_3d_descriptive_only",
        })
    }
}

#[derive(Debug, Error)]
pub enum Spatial3dError {
    #[error("invalid 3-D specification: {0}")]
    Invalid(String),
    #[error("3-D resource limit exceeded: {0}")]
    Resource(String),
    #[error("3-D numerical failure: {0}")]
    Numerical(String),
}

struct Pair {
    left: usize,
    right: usize,
    displacement: [f64; 3],
    distance: f64,
}

pub fn homogeneous_k3d(spec: HomogeneousK3dSpec) -> Result<HomogeneousK3dResult, Spatial3dError> {
    let scale = spec.coordinate_unit.micrometer_scale();
    let (points, window, pair_count, work) = validate_and_normalize(&spec, scale)?;
    let metric =
        spec.anisotropy_matrix
            .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    validate_spd(metric)?;
    let mut pairs = Vec::with_capacity(pair_count as usize);
    for left in 0..points.len() {
        for right in (left + 1)..points.len() {
            let displacement = [
                points[right].coordinates_um[0] - points[left].coordinates_um[0],
                points[right].coordinates_um[1] - points[left].coordinates_um[1],
                points[right].coordinates_um[2] - points[left].coordinates_um[2],
            ];
            let transformed = (0..3)
                .map(|row| {
                    (0..3)
                        .map(|col| metric[row][col] * displacement[col])
                        .sum::<f64>()
                })
                .collect::<Vec<_>>();
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
            pairs.push(Pair {
                left,
                right,
                displacement,
                distance: squared.sqrt(),
            });
        }
    }
    let mut curve = Vec::with_capacity(spec.radii_um.len());
    for &radius in &spec.radii_um {
        let (sum, eligible, border_references, denominator) = match spec.correction {
            K3dCorrection::None => {
                let eligible = pairs.iter().filter(|pair| pair.distance <= radius).count() as u64;
                (
                    2.0 * eligible as f64,
                    2 * eligible,
                    None,
                    (points.len() * (points.len() - 1)) as f64,
                )
            }
            K3dCorrection::Translation => {
                let mut sum = 0.0;
                let mut eligible = 0_u64;
                for pair in pairs.iter().filter(|pair| pair.distance <= radius) {
                    let overlap = (0..3)
                        .map(|axis| window.side_lengths_um[axis] - pair.displacement[axis].abs())
                        .product::<f64>();
                    if !overlap.is_finite() || overlap <= 0.0 {
                        return Err(Spatial3dError::Numerical(
                            "translation overlap is zero for an eligible pair".into(),
                        ));
                    }
                    sum += 2.0 * window.volume_um3 / overlap;
                    eligible += 2;
                }
                (
                    sum,
                    eligible,
                    None,
                    (points.len() * (points.len() - 1)) as f64,
                )
            }
            K3dCorrection::Border => {
                let references = points
                    .iter()
                    .enumerate()
                    .filter_map(|(index, point)| {
                        (boundary_distance(point.coordinates_um, &window) >= radius)
                            .then_some(index)
                    })
                    .collect::<HashSet<_>>();
                if references.is_empty() {
                    return Err(Spatial3dError::Invalid(format!(
                        "border correction has no reference point at radius {radius}"
                    )));
                }
                let mut eligible = 0_u64;
                for pair in pairs.iter().filter(|pair| pair.distance <= radius) {
                    eligible += u64::from(references.contains(&pair.left));
                    eligible += u64::from(references.contains(&pair.right));
                }
                (
                    eligible as f64,
                    eligible,
                    Some(references.len()),
                    (references.len() * (points.len() - 1)) as f64,
                )
            }
        };
        let k = window.volume_um3 * sum / denominator;
        let l3 = (3.0 * k / (4.0 * std::f64::consts::PI)).cbrt();
        if !k.is_finite() || !l3.is_finite() {
            return Err(Spatial3dError::Numerical("K/L result is not finite".into()));
        }
        curve.push(K3dCurvePoint {
            radius_um: radius,
            k_um3: k,
            l3_um: l3,
            eligible_directed_pairs: eligible,
            border_reference_points: border_references,
        });
    }
    Ok(HomogeneousK3dResult {
        format: "marklab.homogeneous_k3d",
        version: 1,
        dimension: 3,
        coordinate_unit: "micrometer",
        normalized_voxel_spacing_um: spec.voxel_spacing.map(|value| value * scale),
        metric: if spec.anisotropy_matrix.is_some() {
            "anisotropic_mahalanobis_physical_um"
        } else {
            "euclidean_physical_um"
        },
        anisotropy_matrix: spec.anisotropy_matrix,
        correction: spec.correction,
        window,
        normalized_points: points,
        curve,
        unordered_pairs_visited: pair_count,
        pair_radius_evaluations: work,
        claim_status: "homogeneous_cuboid_3d_descriptive_only",
    })
}

fn validate_and_normalize(
    spec: &HomogeneousK3dSpec,
    scale: f64,
) -> Result<(Vec<NormalizedPoint3D>, CompiledCuboid3D, u64, u64), Spatial3dError> {
    if spec.points.len() < 2 {
        return Err(Spatial3dError::Invalid(
            "at least two 3-D points are required".into(),
        ));
    }
    if spec
        .voxel_spacing
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(Spatial3dError::Invalid(
            "voxel spacing must be positive and finite".into(),
        ));
    }
    if spec.radii_um.is_empty()
        || spec
            .radii_um
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        || spec.radii_um.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(Spatial3dError::Invalid(
            "radii_um must be finite, nonnegative, nonempty, and strictly increasing".into(),
        ));
    }
    let minimum_um = spec.window.minimum.map(|value| value * scale);
    let maximum_um = spec.window.maximum.map(|value| value * scale);
    let side_lengths_um = [
        maximum_um[0] - minimum_um[0],
        maximum_um[1] - minimum_um[1],
        maximum_um[2] - minimum_um[2],
    ];
    if minimum_um
        .iter()
        .chain(&maximum_um)
        .any(|value| !value.is_finite())
        || side_lengths_um
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(Spatial3dError::Invalid(
            "cuboid bounds must be finite and ordered".into(),
        ));
    }
    let volume_um3 = side_lengths_um.iter().product::<f64>();
    let surface_area_um2 = 2.0
        * (side_lengths_um[0] * side_lengths_um[1]
            + side_lengths_um[0] * side_lengths_um[2]
            + side_lengths_um[1] * side_lengths_um[2]);
    if !volume_um3.is_finite() || !surface_area_um2.is_finite() {
        return Err(Spatial3dError::Invalid(
            "cuboid measures must be finite".into(),
        ));
    }
    let mut ids = HashSet::with_capacity(spec.points.len());
    let mut points = Vec::with_capacity(spec.points.len());
    for point in &spec.points {
        let id = point.id.trim();
        if id.is_empty() || !ids.insert(id.to_owned()) {
            return Err(Spatial3dError::Invalid(
                "point IDs must be unique and nonempty".into(),
            ));
        }
        let coordinates_um = point.coordinates.map(|value| value * scale);
        if coordinates_um.iter().enumerate().any(|(axis, value)| {
            !value.is_finite() || *value < minimum_um[axis] || *value > maximum_um[axis]
        }) {
            return Err(Spatial3dError::Invalid(format!(
                "point {id} lies outside the 3-D cuboid"
            )));
        }
        points.push(NormalizedPoint3D {
            id: id.to_owned(),
            coordinates_um,
        });
    }
    let n = u64::try_from(points.len())
        .map_err(|_| Spatial3dError::Resource("point count overflowed".into()))?;
    let pair_count = n
        .checked_mul(n - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| Spatial3dError::Resource("pair count overflowed".into()))?;
    if pair_count > spec.maximum_unordered_pairs {
        return Err(Spatial3dError::Resource(format!(
            "unordered pairs {pair_count} exceed declared maximum {}",
            spec.maximum_unordered_pairs
        )));
    }
    if pair_count > MAXIMUM_RETAINED_UNORDERED_PAIRS {
        return Err(Spatial3dError::Resource(format!(
            "unordered pairs {pair_count} exceed the built-in retained-pair maximum {MAXIMUM_RETAINED_UNORDERED_PAIRS}"
        )));
    }
    let work = pair_count
        .checked_mul(spec.radii_um.len() as u64)
        .ok_or_else(|| Spatial3dError::Resource("pair-radius work overflowed".into()))?;
    if work > MAXIMUM_PAIR_RADIUS_EVALUATIONS {
        return Err(Spatial3dError::Resource(
            "pair-radius work exceeds 250 million".into(),
        ));
    }
    Ok((
        points,
        CompiledCuboid3D {
            representation: "axis_aligned_cuboid",
            minimum_um,
            maximum_um,
            side_lengths_um,
            volume_um3,
            surface_area_um2,
            components: 1,
            cavities: 0,
        },
        pair_count,
        work,
    ))
}

fn validate_spd(matrix: [[f64; 3]; 3]) -> Result<(), Spatial3dError> {
    if matrix.iter().flatten().any(|value| !value.is_finite()) {
        return Err(Spatial3dError::Invalid(
            "anisotropy matrix must be finite".into(),
        ));
    }
    for (row_index, row) in matrix.iter().enumerate() {
        for (column_index, value) in row.iter().take(row_index).enumerate() {
            if (*value - matrix[column_index][row_index]).abs() > 1.0e-10 {
                return Err(Spatial3dError::Invalid(
                    "anisotropy matrix must be symmetric".into(),
                ));
            }
        }
    }
    let l00 = matrix[0][0].sqrt();
    if !l00.is_finite() || l00 <= 0.0 {
        return Err(Spatial3dError::Invalid(
            "anisotropy matrix must be positive definite".into(),
        ));
    }
    let l10 = matrix[1][0] / l00;
    let l20 = matrix[2][0] / l00;
    let l11 = (matrix[1][1] - l10 * l10).sqrt();
    if !l11.is_finite() || l11 <= 0.0 {
        return Err(Spatial3dError::Invalid(
            "anisotropy matrix must be positive definite".into(),
        ));
    }
    let l21 = (matrix[2][1] - l20 * l10) / l11;
    let l22 = (matrix[2][2] - l20 * l20 - l21 * l21).sqrt();
    if !l22.is_finite() || l22 <= 0.0 {
        return Err(Spatial3dError::Invalid(
            "anisotropy matrix must be positive definite".into(),
        ));
    }
    Ok(())
}

fn boundary_distance(point: [f64; 3], window: &CompiledCuboid3D) -> f64 {
    (0..3)
        .flat_map(|axis| {
            [
                point[axis] - window.minimum_um[axis],
                window.maximum_um[axis] - point[axis],
            ]
        })
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anisotropy_changes_physical_pair_eligibility() {
        let result = homogeneous_k3d(spec(Some([
            [4.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ])))
        .unwrap();
        assert_eq!(result.metric, "anisotropic_mahalanobis_physical_um");
        assert_eq!(result.curve[0].eligible_directed_pairs, 0);
        assert_eq!(result.curve[1].eligible_directed_pairs, 2);
    }

    #[test]
    fn non_positive_definite_anisotropy_is_rejected() {
        let error = homogeneous_k3d(spec(Some([
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
        ])))
        .unwrap_err();
        assert!(error.to_string().contains("positive definite"));
    }

    fn spec(anisotropy_matrix: Option<[[f64; 3]; 3]>) -> HomogeneousK3dSpec {
        HomogeneousK3dSpec {
            points: vec![
                Point3DInput {
                    id: "a".into(),
                    coordinates: [4.0, 5.0, 5.0],
                },
                Point3DInput {
                    id: "b".into(),
                    coordinates: [5.0, 5.0, 5.0],
                },
            ],
            window: CuboidWindowInput {
                minimum: [0.0; 3],
                maximum: [10.0; 3],
            },
            coordinate_unit: CoordinateUnit::Micrometer,
            voxel_spacing: [1.0; 3],
            anisotropy_matrix,
            radii_um: vec![1.0, 2.0],
            correction: K3dCorrection::None,
            maximum_unordered_pairs: 1,
        }
    }
}

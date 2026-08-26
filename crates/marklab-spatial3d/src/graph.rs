use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    validate_and_normalize, validate_spd, CoordinateUnit, CuboidWindowInput, HomogeneousK3dSpec,
    K3dCorrection, Point3DInput, Spatial3dError,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph3dPoint {
    pub id: String,
    pub coordinates: [f64; 3],
    pub position_uncertainty_um: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphRule3D {
    Radius,
    KnnUnion,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceBasis3D {
    Nominal,
    Possible,
    Guaranteed,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphWeight3D {
    Binary,
    Gaussian,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialGraph3dSpec {
    pub points: Vec<Graph3dPoint>,
    pub window: CuboidWindowInput,
    pub coordinate_unit: CoordinateUnit,
    pub voxel_spacing: [f64; 3],
    pub anisotropy_matrix: Option<[[f64; 3]; 3]>,
    pub rule: GraphRule3D,
    pub radius_um: Option<f64>,
    pub neighbors: Option<usize>,
    pub distance_basis: DistanceBasis3D,
    pub weight: GraphWeight3D,
    pub gaussian_bandwidth_um: Option<f64>,
    pub maximum_unordered_pairs: u64,
    pub maximum_edges: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalizedGraph3dPoint {
    pub id: String,
    pub coordinates_um: [f64; 3],
    pub position_uncertainty_um: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Graph3dEdge {
    pub left_id: String,
    pub right_id: String,
    pub nominal_distance_um: f64,
    pub effective_distance_um: f64,
    pub combined_position_uncertainty_um: f64,
    pub weight: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SparseGraphOperator3D {
    pub node_ids: Vec<String>,
    pub row_offsets: Vec<usize>,
    pub column_indices: Vec<usize>,
    pub weights: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpatialGraph3dResult {
    pub format: &'static str,
    pub version: u32,
    pub metric: &'static str,
    pub rule: GraphRule3D,
    pub distance_basis: DistanceBasis3D,
    pub weight_kind: GraphWeight3D,
    pub radius_um: Option<f64>,
    pub neighbors: Option<usize>,
    pub gaussian_bandwidth_um: Option<f64>,
    pub normalized_voxel_spacing_um: [f64; 3],
    pub anisotropy_matrix: Option<[[f64; 3]; 3]>,
    pub normalized_points: Vec<NormalizedGraph3dPoint>,
    pub edges: Vec<Graph3dEdge>,
    pub sparse_operator: SparseGraphOperator3D,
    pub unordered_pairs_visited: u64,
    pub graph_sha256: String,
    pub claim_status: &'static str,
}

struct Candidate {
    left: usize,
    right: usize,
    nominal: f64,
    effective: f64,
    uncertainty: f64,
}

pub fn build_spatial_graph3d(
    spec: SpatialGraph3dSpec,
) -> Result<SpatialGraph3dResult, Spatial3dError> {
    validate_graph_settings(&spec)?;
    let base = HomogeneousK3dSpec {
        points: spec
            .points
            .iter()
            .map(|point| Point3DInput {
                id: point.id.clone(),
                coordinates: point.coordinates,
            })
            .collect(),
        window: spec.window.clone(),
        coordinate_unit: spec.coordinate_unit,
        voxel_spacing: spec.voxel_spacing,
        anisotropy_matrix: spec.anisotropy_matrix,
        radii_um: vec![0.0],
        correction: K3dCorrection::None,
        maximum_unordered_pairs: spec.maximum_unordered_pairs,
    };
    let scale = base.coordinate_unit.micrometer_scale();
    let (normalized, _, pair_count, _) = validate_and_normalize(&base, scale)?;
    let mut points = normalized
        .into_iter()
        .zip(&spec.points)
        .map(|(point, source)| NormalizedGraph3dPoint {
            id: point.id,
            coordinates_um: point.coordinates_um,
            position_uncertainty_um: source.position_uncertainty_um,
        })
        .collect::<Vec<_>>();
    points.sort_by(|left, right| left.id.cmp(&right.id));
    let metric =
        base.anisotropy_matrix
            .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    validate_spd(metric)?;
    let mut candidates = Vec::with_capacity(pair_count as usize);
    for left in 0..points.len() {
        for right in (left + 1)..points.len() {
            let displacement = subtract(points[right].coordinates_um, points[left].coordinates_um);
            let nominal = distance(displacement, metric)?;
            let uncertainty =
                points[left].position_uncertainty_um + points[right].position_uncertainty_um;
            let effective = match spec.distance_basis {
                DistanceBasis3D::Nominal => nominal,
                DistanceBasis3D::Possible => (nominal - uncertainty).max(0.0),
                DistanceBasis3D::Guaranteed => nominal + uncertainty,
            };
            candidates.push(Candidate {
                left,
                right,
                nominal,
                effective,
                uncertainty,
            });
        }
    }
    let selected = select_edges(&spec, &points, &candidates)?;
    if selected.len() > spec.maximum_edges {
        return Err(Spatial3dError::Resource(format!(
            "selected edges {} exceed declared maximum {}",
            selected.len(),
            spec.maximum_edges
        )));
    }
    let mut edges = Vec::with_capacity(selected.len());
    for candidate_index in selected {
        let candidate = &candidates[candidate_index];
        let weight = match spec.weight {
            GraphWeight3D::Binary => 1.0,
            GraphWeight3D::Gaussian => {
                let bandwidth = spec
                    .gaussian_bandwidth_um
                    .expect("validated Gaussian bandwidth");
                (-0.5 * (candidate.effective / bandwidth).powi(2)).exp()
            }
        };
        edges.push(Graph3dEdge {
            left_id: points[candidate.left].id.clone(),
            right_id: points[candidate.right].id.clone(),
            nominal_distance_um: candidate.nominal,
            effective_distance_um: candidate.effective,
            combined_position_uncertainty_um: candidate.uncertainty,
            weight,
        });
    }
    let sparse_operator = sparse_operator(&points, &edges);
    let graph_sha256 = graph_digest(&spec, &points, &edges);
    Ok(SpatialGraph3dResult {
        format: "marklab.spatial_graph3d",
        version: 1,
        metric: if base.anisotropy_matrix.is_some() {
            "anisotropic_mahalanobis_physical_um"
        } else {
            "euclidean_physical_um"
        },
        rule: spec.rule,
        distance_basis: spec.distance_basis,
        weight_kind: spec.weight,
        radius_um: spec.radius_um,
        neighbors: spec.neighbors,
        gaussian_bandwidth_um: spec.gaussian_bandwidth_um,
        normalized_voxel_spacing_um: spec
            .voxel_spacing
            .map(|value| value * spec.coordinate_unit.micrometer_scale()),
        anisotropy_matrix: spec.anisotropy_matrix,
        normalized_points: points,
        edges,
        sparse_operator,
        unordered_pairs_visited: pair_count,
        graph_sha256,
        claim_status: "bounded_physical_3d_graph_descriptive_only",
    })
}

fn validate_graph_settings(spec: &SpatialGraph3dSpec) -> Result<(), Spatial3dError> {
    if spec.points.iter().any(|point| {
        !point.position_uncertainty_um.is_finite() || point.position_uncertainty_um < 0.0
    }) {
        return Err(Spatial3dError::Invalid(
            "position uncertainties must be nonnegative finite micrometre bounds".into(),
        ));
    }
    match spec.rule {
        GraphRule3D::Radius => {
            if spec.neighbors.is_some()
                || spec
                    .radius_um
                    .is_none_or(|radius| !radius.is_finite() || radius < 0.0)
            {
                return Err(Spatial3dError::Invalid(
                    "radius rule requires only a nonnegative finite radius_um".into(),
                ));
            }
        }
        GraphRule3D::KnnUnion => {
            if spec.radius_um.is_some()
                || spec
                    .neighbors
                    .is_none_or(|neighbors| neighbors == 0 || neighbors >= spec.points.len())
            {
                return Err(Spatial3dError::Invalid(
                    "knn_union requires only neighbors in 1..point_count".into(),
                ));
            }
        }
    }
    match spec.weight {
        GraphWeight3D::Binary if spec.gaussian_bandwidth_um.is_some() => Err(
            Spatial3dError::Invalid("binary weight must not declare a Gaussian bandwidth".into()),
        ),
        GraphWeight3D::Gaussian
            if spec
                .gaussian_bandwidth_um
                .is_none_or(|bandwidth| !bandwidth.is_finite() || bandwidth <= 0.0) =>
        {
            Err(Spatial3dError::Invalid(
                "Gaussian weight requires a positive finite bandwidth".into(),
            ))
        }
        _ => Ok(()),
    }
}

fn select_edges(
    spec: &SpatialGraph3dSpec,
    points: &[NormalizedGraph3dPoint],
    candidates: &[Candidate],
) -> Result<BTreeSet<usize>, Spatial3dError> {
    match spec.rule {
        GraphRule3D::Radius => Ok(candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (candidate.effective <= spec.radius_um.expect("validated radius")).then_some(index)
            })
            .collect()),
        GraphRule3D::KnnUnion => {
            let neighbors = spec.neighbors.expect("validated neighbor count");
            let mut selected = BTreeSet::new();
            for node in 0..points.len() {
                let mut incident = candidates
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        if candidate.left == node {
                            Some((index, candidate.effective, &points[candidate.right].id))
                        } else if candidate.right == node {
                            Some((index, candidate.effective, &points[candidate.left].id))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                incident.sort_by(|left, right| {
                    left.1.total_cmp(&right.1).then_with(|| left.2.cmp(right.2))
                });
                selected.extend(incident.into_iter().take(neighbors).map(|entry| entry.0));
            }
            Ok(selected)
        }
    }
}

fn sparse_operator(
    points: &[NormalizedGraph3dPoint],
    edges: &[Graph3dEdge],
) -> SparseGraphOperator3D {
    let mut neighbors = vec![Vec::<(usize, f64)>::new(); points.len()];
    for edge in edges {
        let left = points
            .binary_search_by(|point| point.id.cmp(&edge.left_id))
            .expect("edge endpoints come from normalized points");
        let right = points
            .binary_search_by(|point| point.id.cmp(&edge.right_id))
            .expect("edge endpoints come from normalized points");
        neighbors[left].push((right, edge.weight));
        neighbors[right].push((left, edge.weight));
    }
    for row in &mut neighbors {
        row.sort_by_key(|entry| entry.0);
    }
    let mut row_offsets = Vec::with_capacity(points.len() + 1);
    let mut column_indices = Vec::with_capacity(edges.len() * 2);
    let mut weights = Vec::with_capacity(edges.len() * 2);
    row_offsets.push(0);
    for row in neighbors {
        for (column, weight) in row {
            column_indices.push(column);
            weights.push(weight);
        }
        row_offsets.push(column_indices.len());
    }
    SparseGraphOperator3D {
        node_ids: points.iter().map(|point| point.id.clone()).collect(),
        row_offsets,
        column_indices,
        weights,
    }
}

fn graph_digest(
    spec: &SpatialGraph3dSpec,
    points: &[NormalizedGraph3dPoint],
    edges: &[Graph3dEdge],
) -> String {
    let mut digest = Sha256::new();
    update_string(&mut digest, "marklab.spatial_graph3d.digest.v1");
    update_string(
        &mut digest,
        match spec.rule {
            GraphRule3D::Radius => "radius",
            GraphRule3D::KnnUnion => "knn_union",
        },
    );
    update_string(
        &mut digest,
        match spec.distance_basis {
            DistanceBasis3D::Nominal => "nominal",
            DistanceBasis3D::Possible => "possible",
            DistanceBasis3D::Guaranteed => "guaranteed",
        },
    );
    update_string(
        &mut digest,
        match spec.weight {
            GraphWeight3D::Binary => "binary",
            GraphWeight3D::Gaussian => "gaussian",
        },
    );
    update_optional_f64(&mut digest, spec.radius_um);
    update_optional_u64(&mut digest, spec.neighbors.map(|value| value as u64));
    update_optional_f64(&mut digest, spec.gaussian_bandwidth_um);
    let scale = spec.coordinate_unit.micrometer_scale();
    for value in spec
        .window
        .minimum
        .iter()
        .chain(&spec.window.maximum)
        .chain(&spec.voxel_spacing)
    {
        digest.update((value * scale).to_bits().to_be_bytes());
    }
    let metric =
        spec.anisotropy_matrix
            .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    for value in metric.iter().flatten() {
        digest.update(value.to_bits().to_be_bytes());
    }
    for point in points {
        update_string(&mut digest, &point.id);
        for value in point.coordinates_um {
            digest.update(value.to_bits().to_be_bytes());
        }
        digest.update(point.position_uncertainty_um.to_bits().to_be_bytes());
    }
    for edge in edges {
        update_string(&mut digest, &edge.left_id);
        update_string(&mut digest, &edge.right_id);
        digest.update(edge.nominal_distance_um.to_bits().to_be_bytes());
        digest.update(edge.effective_distance_um.to_bits().to_be_bytes());
        digest.update(
            edge.combined_position_uncertainty_um
                .to_bits()
                .to_be_bytes(),
        );
        digest.update(edge.weight.to_bits().to_be_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn update_string(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}

fn update_optional_f64(digest: &mut Sha256, value: Option<f64>) {
    match value {
        Some(value) => {
            digest.update([1]);
            digest.update(value.to_bits().to_be_bytes());
        }
        None => digest.update([0]),
    }
}

fn update_optional_u64(digest: &mut Sha256, value: Option<u64>) {
    match value {
        Some(value) => {
            digest.update([1]);
            digest.update(value.to_be_bytes());
        }
        None => digest.update([0]),
    }
}

fn subtract(right: [f64; 3], left: [f64; 3]) -> [f64; 3] {
    [right[0] - left[0], right[1] - left[1], right[2] - left[2]]
}

fn distance(displacement: [f64; 3], metric: [[f64; 3]; 3]) -> Result<f64, Spatial3dError> {
    let transformed = metric.map(|row| {
        row.iter()
            .zip(displacement)
            .map(|(coefficient, value)| coefficient * value)
            .sum::<f64>()
    });
    let squared = displacement
        .iter()
        .zip(transformed)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    if !squared.is_finite() || squared < 0.0 {
        return Err(Spatial3dError::Numerical(
            "graph squared distance is negative or non-finite".into(),
        ));
    }
    Ok(squared.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncertainty_basis_controls_admission_and_digest_ignores_row_order() {
        let mut possible = spec(DistanceBasis3D::Possible);
        let first = build_spatial_graph3d(possible.clone()).unwrap();
        possible.points.reverse();
        let reversed = build_spatial_graph3d(possible).unwrap();
        assert_eq!(first.edges.len(), 1);
        assert_eq!(first.edges[0].effective_distance_um, 1.2);
        assert_eq!(first.graph_sha256, reversed.graph_sha256);

        let mut wider_radius = spec(DistanceBasis3D::Possible);
        wider_radius.radius_um = Some(2.5);
        let wider_radius = build_spatial_graph3d(wider_radius).unwrap();
        assert_eq!(first.edges.len(), wider_radius.edges.len());
        assert_ne!(first.graph_sha256, wider_radius.graph_sha256);

        let guaranteed = build_spatial_graph3d(spec(DistanceBasis3D::Guaranteed)).unwrap();
        assert!(guaranteed.edges.is_empty());
    }

    fn spec(distance_basis: DistanceBasis3D) -> SpatialGraph3dSpec {
        SpatialGraph3dSpec {
            points: vec![
                Graph3dPoint {
                    id: "a".into(),
                    coordinates: [0.0, 1.0, 1.0],
                    position_uncertainty_um: 0.4,
                },
                Graph3dPoint {
                    id: "b".into(),
                    coordinates: [2.0, 1.0, 1.0],
                    position_uncertainty_um: 0.4,
                },
            ],
            window: CuboidWindowInput {
                minimum: [0.0; 3],
                maximum: [10.0; 3],
            },
            coordinate_unit: CoordinateUnit::Micrometer,
            voxel_spacing: [1.0; 3],
            anisotropy_matrix: None,
            rule: GraphRule3D::Radius,
            radius_um: Some(1.5),
            neighbors: None,
            distance_basis,
            weight: GraphWeight3D::Binary,
            gaussian_bandwidth_um: None,
            maximum_unordered_pairs: 1,
            maximum_edges: 1,
        }
    }
}

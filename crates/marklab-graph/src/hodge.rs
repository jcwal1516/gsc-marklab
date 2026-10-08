use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::GraphError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HodgeEdgeInput {
    pub source_id: String,
    pub target_id: String,
    pub flow: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimplicialHodgeSpec {
    pub node_ids: Vec<String>,
    pub edges: Vec<HodgeEdgeInput>,
    pub maximum_dimension: usize,
    pub filter_step: f64,
    pub orthogonality_tolerance: f64,
    pub maximum_clique_triples: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CanonicalHodgeEdge {
    pub source_id: String,
    pub target_id: String,
    pub flow: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SimplicialInventory {
    pub nodes: Vec<String>,
    pub edges: Vec<[String; 2]>,
    pub triangles: Vec<[String; 3]>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HodgeDecompositionResult {
    pub gradient_component: Vec<f64>,
    pub curl_component: Vec<f64>,
    pub harmonic_component: Vec<f64>,
    pub gradient_curl_dot: f64,
    pub gradient_harmonic_dot: f64,
    pub curl_harmonic_dot: f64,
    pub gradient_norm: f64,
    pub curl_norm: f64,
    pub harmonic_norm: f64,
    pub reconstruction_max_abs_error: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SimplicialHodgeResult {
    pub format: &'static str,
    pub version: u32,
    pub complex_digest: String,
    pub simplices: SimplicialInventory,
    pub canonical_edges: Vec<CanonicalHodgeEdge>,
    pub boundary_1: Vec<Vec<f64>>,
    pub boundary_2: Vec<Vec<f64>>,
    pub boundary_of_boundary_max_abs: f64,
    pub hodge_lower_1: Vec<Vec<f64>>,
    pub hodge_upper_1: Vec<Vec<f64>>,
    pub hodge_laplacian_1: Vec<Vec<f64>>,
    pub decomposition: HodgeDecompositionResult,
    pub filter: &'static str,
    pub filter_step: f64,
    pub filtered_flow: Vec<f64>,
    pub clique_triples_evaluated: u64,
    pub claim_status: &'static str,
}

pub fn simplicial_hodge_workflow(
    mut spec: SimplicialHodgeSpec,
) -> Result<SimplicialHodgeResult, GraphError> {
    validate(&spec)?;
    spec.node_ids.sort();
    let node_index = spec
        .node_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut edge_map = BTreeMap::<(usize, usize), f64>::new();
    for edge in spec.edges {
        let source = node_index
            .get(edge.source_id.as_str())
            .copied()
            .ok_or_else(|| {
                GraphError::Invalid(format!("unknown Hodge edge node {}", edge.source_id))
            })?;
        let target = node_index
            .get(edge.target_id.as_str())
            .copied()
            .ok_or_else(|| {
                GraphError::Invalid(format!("unknown Hodge edge node {}", edge.target_id))
            })?;
        if source == target {
            return Err(GraphError::Invalid("Hodge graph forbids self edges".into()));
        }
        let (pair, flow) = if source < target {
            ((source, target), edge.flow)
        } else {
            ((target, source), -edge.flow)
        };
        if edge_map.insert(pair, flow).is_some() {
            return Err(GraphError::Invalid("duplicate Hodge edge".into()));
        }
    }
    let edges = edge_map.keys().copied().collect::<Vec<_>>();
    let edge_index = edges
        .iter()
        .enumerate()
        .map(|(index, edge)| (*edge, index))
        .collect::<BTreeMap<_, _>>();
    let flow = edges.iter().map(|edge| edge_map[edge]).collect::<Vec<_>>();
    let triples_evaluated = choose_three(spec.node_ids.len())?;
    if triples_evaluated > spec.maximum_clique_triples {
        return Err(GraphError::Invalid(format!(
            "clique triple work {triples_evaluated} exceeds caller maximum {}",
            spec.maximum_clique_triples
        )));
    }
    let edge_set = edges.iter().copied().collect::<HashSet<_>>();
    let mut triangles = Vec::<[usize; 3]>::new();
    for first in 0..spec.node_ids.len() {
        for second in (first + 1)..spec.node_ids.len() {
            for third in (second + 1)..spec.node_ids.len() {
                if edge_set.contains(&(first, second))
                    && edge_set.contains(&(first, third))
                    && edge_set.contains(&(second, third))
                {
                    triangles.push([first, second, third]);
                }
            }
        }
    }
    let mut boundary_1 = vec![vec![0.0; edges.len()]; spec.node_ids.len()];
    for (column, &(source, target)) in edges.iter().enumerate() {
        boundary_1[source][column] = -1.0;
        boundary_1[target][column] = 1.0;
    }
    let mut boundary_2 = vec![vec![0.0; triangles.len()]; edges.len()];
    for (column, &[a, b, c]) in triangles.iter().enumerate() {
        boundary_2[edge_index[&(b, c)]][column] = 1.0;
        boundary_2[edge_index[&(a, c)]][column] = -1.0;
        boundary_2[edge_index[&(a, b)]][column] = 1.0;
    }
    let boundary_of_boundary = multiply(&boundary_1, &boundary_2);
    let boundary_of_boundary_max_abs = boundary_of_boundary
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if boundary_of_boundary_max_abs > 1e-12 {
        return Err(GraphError::Numerical(
            "simplicial boundary of boundary is nonzero".into(),
        ));
    }
    let hodge_lower_1 = multiply(&transpose(&boundary_1), &boundary_1);
    let hodge_upper_1 = if triangles.is_empty() {
        vec![vec![0.0; edges.len()]; edges.len()]
    } else {
        multiply(&boundary_2, &transpose(&boundary_2))
    };
    let hodge_laplacian_1 = add(&hodge_lower_1, &hodge_upper_1);
    let gradient_rhs = matrix_vector(&boundary_1, &flow);
    let node_laplacian = multiply(&boundary_1, &transpose(&boundary_1));
    let potential = solve_with_last_zero_gauge(&node_laplacian, &gradient_rhs)?;
    let gradient_component = matrix_vector(&transpose(&boundary_1), &potential);
    let curl_component = if triangles.is_empty() {
        vec![0.0; edges.len()]
    } else {
        let transpose_b2 = transpose(&boundary_2);
        let curl_gram = multiply(&transpose_b2, &boundary_2);
        let curl_rhs = matrix_vector(&transpose_b2, &flow);
        let curl_potential = solve(&curl_gram, &curl_rhs)?;
        matrix_vector(&boundary_2, &curl_potential)
    };
    let harmonic_component = flow
        .iter()
        .zip(&gradient_component)
        .zip(&curl_component)
        .map(|((flow, gradient), curl)| flow - gradient - curl)
        .collect::<Vec<_>>();
    let gradient_curl_dot = dot(&gradient_component, &curl_component);
    let gradient_harmonic_dot = dot(&gradient_component, &harmonic_component);
    let curl_harmonic_dot = dot(&curl_component, &harmonic_component);
    let gradient_norm = dot(&gradient_component, &gradient_component).sqrt();
    let curl_norm = dot(&curl_component, &curl_component).sqrt();
    let harmonic_norm = dot(&harmonic_component, &harmonic_component).sqrt();
    for (value, scale) in [
        (gradient_curl_dot, gradient_norm * curl_norm),
        (gradient_harmonic_dot, gradient_norm * harmonic_norm),
        (curl_harmonic_dot, curl_norm * harmonic_norm),
    ] {
        if value.abs() > spec.orthogonality_tolerance * scale.max(1.0) {
            return Err(GraphError::Numerical(
                "Hodge components violate declared orthogonality tolerance".into(),
            ));
        }
    }
    let reconstruction_max_abs_error = flow
        .iter()
        .zip(&gradient_component)
        .zip(&curl_component)
        .zip(&harmonic_component)
        .map(|(((flow, gradient), curl), harmonic)| (flow - gradient - curl - harmonic).abs())
        .fold(0.0_f64, f64::max);
    let filtered_flow = flow
        .iter()
        .zip(matrix_vector(&hodge_laplacian_1, &flow))
        .map(|(flow, applied)| flow - spec.filter_step * applied)
        .collect::<Vec<_>>();
    let canonical_edges = edges
        .iter()
        .zip(&flow)
        .map(|(&(source, target), &flow)| CanonicalHodgeEdge {
            source_id: spec.node_ids[source].clone(),
            target_id: spec.node_ids[target].clone(),
            flow,
        })
        .collect::<Vec<_>>();
    let simplices = SimplicialInventory {
        nodes: spec.node_ids.clone(),
        edges: edges
            .iter()
            .map(|&(source, target)| [spec.node_ids[source].clone(), spec.node_ids[target].clone()])
            .collect(),
        triangles: triangles
            .iter()
            .map(|triangle| triangle.map(|index| spec.node_ids[index].clone()))
            .collect(),
    };
    let digest = serde_json::to_vec(&serde_json::json!({
        "simplices": &simplices,
        "boundary_1": &boundary_1,
        "boundary_2": &boundary_2,
    }))
    .map_err(|error| GraphError::Numerical(error.to_string()))?;
    Ok(SimplicialHodgeResult {
        format: "marklab.simplicial_hodge",
        version: 1,
        complex_digest: format!("{:x}", Sha256::digest(digest)),
        simplices,
        canonical_edges,
        boundary_1,
        boundary_2,
        boundary_of_boundary_max_abs,
        hodge_lower_1,
        hodge_upper_1,
        hodge_laplacian_1,
        decomposition: HodgeDecompositionResult {
            gradient_component,
            curl_component,
            harmonic_component,
            gradient_curl_dot,
            gradient_harmonic_dot,
            curl_harmonic_dot,
            gradient_norm,
            curl_norm,
            harmonic_norm,
            reconstruction_max_abs_error,
        },
        filter: "first_order_polynomial_identity_minus_step_l1",
        filter_step: spec.filter_step,
        filtered_flow,
        clique_triples_evaluated: triples_evaluated,
        claim_status: "experimental_simplicial_edge_flow",
    })
}

fn validate(spec: &SimplicialHodgeSpec) -> Result<(), GraphError> {
    if !(3..=1_000).contains(&spec.node_ids.len())
        || spec.edges.is_empty()
        || spec.maximum_dimension != 2
        || !spec.filter_step.is_finite()
        || spec.filter_step <= 0.0
        || !spec.orthogonality_tolerance.is_finite()
        || spec.orthogonality_tolerance <= 0.0
        || spec.maximum_clique_triples == 0
    {
        return Err(GraphError::Invalid(
            "Hodge workflow requires bounded graph, dimension two, positive filter/tolerance/work"
                .into(),
        ));
    }
    let mut nodes = HashSet::new();
    if spec
        .node_ids
        .iter()
        .any(|id| id.is_empty() || id.trim() != id || !nodes.insert(id.as_str()))
        || spec.edges.iter().any(|edge| {
            edge.source_id.is_empty() || edge.target_id.is_empty() || !edge.flow.is_finite()
        })
    {
        return Err(GraphError::Invalid(
            "Hodge nodes/edges require unique exact IDs and finite flows".into(),
        ));
    }
    Ok(())
}

fn choose_three(count: usize) -> Result<u64, GraphError> {
    (count as u64)
        .checked_mul(count as u64 - 1)
        .and_then(|value| value.checked_mul(count as u64 - 2))
        .and_then(|value| value.checked_div(6))
        .ok_or_else(|| GraphError::Invalid("clique triple count overflow".into()))
}

fn transpose(matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
    if matrix.is_empty() {
        return Vec::new();
    }
    (0..matrix[0].len())
        .map(|column| matrix.iter().map(|row| row[column]).collect())
        .collect()
}

fn multiply(left: &[Vec<f64>], right: &[Vec<f64>]) -> Vec<Vec<f64>> {
    if left.is_empty() || right.is_empty() {
        return vec![vec![0.0; right.first().map_or(0, Vec::len)]; left.len()];
    }
    (0..left.len())
        .map(|row| {
            (0..right[0].len())
                .map(|column| {
                    left[row]
                        .iter()
                        .zip(right)
                        .map(|(value, right_row)| value * right_row[column])
                        .sum()
                })
                .collect()
        })
        .collect()
}

fn add(left: &[Vec<f64>], right: &[Vec<f64>]) -> Vec<Vec<f64>> {
    left.iter()
        .zip(right)
        .map(|(left, right)| left.iter().zip(right).map(|(a, b)| a + b).collect())
        .collect()
}

fn matrix_vector(matrix: &[Vec<f64>], vector: &[f64]) -> Vec<f64> {
    matrix
        .iter()
        .map(|row| {
            row.iter()
                .zip(vector)
                .map(|(left, right)| left * right)
                .sum()
        })
        .collect()
}

fn solve_with_last_zero_gauge(matrix: &[Vec<f64>], rhs: &[f64]) -> Result<Vec<f64>, GraphError> {
    let reduced = matrix[..matrix.len() - 1]
        .iter()
        .map(|row| row[..row.len() - 1].to_vec())
        .collect::<Vec<_>>();
    let mut result = solve(&reduced, &rhs[..rhs.len() - 1])?;
    result.push(0.0);
    Ok(result)
}

fn solve(matrix: &[Vec<f64>], rhs: &[f64]) -> Result<Vec<f64>, GraphError> {
    if rhs.is_empty() {
        return Ok(Vec::new());
    }
    let size = rhs.len();
    let mut augmented = matrix
        .iter()
        .zip(rhs)
        .map(|(row, rhs)| {
            let mut row = row.clone();
            row.push(*rhs);
            row
        })
        .collect::<Vec<_>>();
    for pivot in 0..size {
        let selected = (pivot..size)
            .max_by(|left, right| {
                augmented[*left][pivot]
                    .abs()
                    .total_cmp(&augmented[*right][pivot].abs())
            })
            .expect("pivot range");
        if augmented[selected][pivot].abs() <= 1e-12 {
            return Err(GraphError::Invalid(
                "Hodge least-squares system is rank deficient for this specialization".into(),
            ));
        }
        augmented.swap(pivot, selected);
        let diagonal = augmented[pivot][pivot];
        for value in &mut augmented[pivot][pivot..=size] {
            *value /= diagonal;
        }
        let pivot_values = augmented[pivot][pivot..=size].to_vec();
        for (row_index, row) in augmented.iter_mut().enumerate() {
            if row_index == pivot {
                continue;
            }
            let factor = row[pivot];
            for (value, pivot_value) in row[pivot..=size].iter_mut().zip(&pivot_values) {
                *value -= factor * pivot_value;
            }
        }
    }
    Ok(augmented.iter().map(|row| row[size]).collect())
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

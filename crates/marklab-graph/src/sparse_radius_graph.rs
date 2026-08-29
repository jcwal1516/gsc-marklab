use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::{GraphError, GraphNodeInput};

pub(crate) struct SparseRadiusGraph {
    pub(crate) nodes: Vec<GraphNodeInput>,
    pub(crate) edges: Vec<(usize, usize)>,
    pub(crate) degrees: Vec<u32>,
    pub(crate) candidate_pair_evaluations: u64,
    pub(crate) cell_count: usize,
}

pub(crate) fn build_sparse_radius_graph(
    mut nodes: Vec<GraphNodeInput>,
    radius_um: f64,
    maximum_nodes: usize,
    maximum_candidate_pairs: u64,
    maximum_edges: u64,
) -> Result<SparseRadiusGraph, GraphError> {
    if nodes.len() < 2
        || maximum_nodes < 2
        || nodes.len() > maximum_nodes
        || !radius_um.is_finite()
        || radius_um <= 0.0
        || !(radius_um * radius_um).is_finite()
        || maximum_candidate_pairs == 0
        || maximum_edges == 0
    {
        return Err(GraphError::Invalid(
            "sparse radius graph controls or resource limits are invalid".into(),
        ));
    }
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let mut ids = BTreeSet::new();
    let mut cells = BTreeMap::<(i64, i64), Vec<usize>>::new();
    for (index, node) in nodes.iter().enumerate() {
        if node.id.is_empty()
            || node.id.trim() != node.id
            || !ids.insert(node.id.as_str())
            || node
                .coordinates_um
                .iter()
                .chain(std::iter::once(&node.signal))
                .any(|value| !value.is_finite())
        {
            return Err(GraphError::Invalid(
                "sparse radius graph nodes require unique exact IDs and finite coordinates/signals"
                    .into(),
            ));
        }
        cells
            .entry(cell_key(node.coordinates_um, radius_um)?)
            .or_default()
            .push(index);
    }

    let radius_squared = radius_um * radius_um;
    let mut candidates = 0_u64;
    let mut edges = Vec::new();
    let mut degrees = vec![0_u32; nodes.len()];
    for (left, node) in nodes.iter().enumerate() {
        let (cell_x, cell_y) = cell_key(node.coordinates_um, radius_um)?;
        for dx in -1_i64..=1 {
            for dy in -1_i64..=1 {
                let (Some(neighbor_x), Some(neighbor_y)) =
                    (cell_x.checked_add(dx), cell_y.checked_add(dy))
                else {
                    continue;
                };
                let Some(neighbors) = cells.get(&(neighbor_x, neighbor_y)) else {
                    continue;
                };
                for &right in neighbors {
                    if right <= left {
                        continue;
                    }
                    candidates = candidates.checked_add(1).ok_or_else(|| {
                        GraphError::Invalid("candidate pair count overflow".into())
                    })?;
                    if candidates > maximum_candidate_pairs {
                        return Err(GraphError::Invalid(
                            "candidate pair count exceeds caller maximum".into(),
                        ));
                    }
                    let right_node = &nodes[right];
                    let delta_x = node.coordinates_um[0] - right_node.coordinates_um[0];
                    let delta_y = node.coordinates_um[1] - right_node.coordinates_um[1];
                    if delta_x.mul_add(delta_x, delta_y * delta_y) <= radius_squared {
                        if edges.len() as u64 >= maximum_edges {
                            return Err(GraphError::Invalid(
                                "edge count exceeds caller maximum".into(),
                            ));
                        }
                        degrees[left] = degrees[left]
                            .checked_add(1)
                            .ok_or_else(|| GraphError::Invalid("node degree overflow".into()))?;
                        degrees[right] = degrees[right]
                            .checked_add(1)
                            .ok_or_else(|| GraphError::Invalid("node degree overflow".into()))?;
                        edges.push((left, right));
                    }
                }
            }
        }
    }
    if edges.is_empty() {
        return Err(GraphError::Invalid(
            "sparse radius graph must contain at least one edge".into(),
        ));
    }
    Ok(SparseRadiusGraph {
        nodes,
        edges,
        degrees,
        candidate_pair_evaluations: candidates,
        cell_count: cells.len(),
    })
}

pub(crate) fn laplacian_product(
    degrees: &[u32],
    edges: &[(usize, usize)],
    input: &[f64],
) -> Vec<f64> {
    let mut output = degrees
        .iter()
        .zip(input)
        .map(|(degree, value)| f64::from(*degree) * value)
        .collect::<Vec<_>>();
    for &(left, right) in edges {
        output[left] -= input[right];
        output[right] -= input[left];
    }
    output
}

pub(crate) fn graph_digest(
    nodes: &[GraphNodeInput],
    edges: &[(usize, usize)],
    radius_um: f64,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"marklab-sparse-radius-graph-v1\0");
    digest.update(radius_um.to_bits().to_be_bytes());
    for node in nodes {
        digest.update((node.id.len() as u64).to_be_bytes());
        digest.update(node.id.as_bytes());
        digest.update(node.coordinates_um[0].to_bits().to_be_bytes());
        digest.update(node.coordinates_um[1].to_bits().to_be_bytes());
    }
    for &(left, right) in edges {
        digest.update((left as u64).to_be_bytes());
        digest.update((right as u64).to_be_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn cell_key(coordinates: [f64; 2], radius_um: f64) -> Result<(i64, i64), GraphError> {
    let scaled = [coordinates[0] / radius_um, coordinates[1] / radius_um];
    if scaled
        .iter()
        .any(|value| !value.is_finite() || *value < i64::MIN as f64 || *value > i64::MAX as f64)
    {
        return Err(GraphError::Invalid(
            "sparse radius graph coordinate cannot be represented by the radius grid".into(),
        ));
    }
    Ok((scaled[0].floor() as i64, scaled[1].floor() as i64))
}

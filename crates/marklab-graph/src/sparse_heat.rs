use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{heat_chebyshev_coefficients, heat_grid_error, GraphError, GraphNodeInput};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusHeatSpec {
    pub nodes: Vec<GraphNodeInput>,
    pub radius_um: f64,
    pub time: f64,
    pub tolerance: f64,
    pub maximum_order: usize,
    pub maximum_nodes: usize,
    pub maximum_candidate_pairs: u64,
    pub maximum_edges: u64,
    pub maximum_matrix_vector_work: u64,
    pub maximum_working_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusHeatResult {
    pub format: String,
    pub version: u32,
    pub graph_digest: String,
    pub graph_rule: String,
    pub laplacian_kind: String,
    pub node_count: usize,
    pub edge_count: u64,
    pub isolated_node_count: usize,
    pub radius_um: f64,
    pub time: f64,
    pub spectral_upper_bound: f64,
    pub selected_order: usize,
    pub coefficients: Vec<f64>,
    pub estimated_tail_bound: f64,
    pub verified_grid_error: f64,
    pub tolerance: f64,
    pub filtered_signal: Vec<f64>,
    pub signal_l2_norm: f64,
    pub diagnostic_l2_error_bound: f64,
    pub candidate_pair_evaluations: u64,
    pub matrix_vector_products: usize,
    pub matrix_vector_work: u64,
    pub working_bytes: u64,
    pub claim_status: String,
}

impl GraphSparseRadiusHeatResult {
    pub fn validate_for_spec(&self, spec: &GraphSparseRadiusHeatSpec) -> Result<(), GraphError> {
        if self.format != "marklab.graph_sparse_radius_heat"
            || self.version != 1
            || self.graph_rule != "uniform_cell_exact_physical_radius"
            || self.laplacian_kind != "combinatorial_binary_sparse"
            || self.claim_status != "experimental_sparse_graph_signal"
            || self.node_count != spec.nodes.len()
            || self.filtered_signal.len() != spec.nodes.len()
            || self.radius_um.to_bits() != spec.radius_um.to_bits()
            || self.time.to_bits() != spec.time.to_bits()
            || self.tolerance.to_bits() != spec.tolerance.to_bits()
            || self.selected_order == 0
            || self.selected_order > spec.maximum_order
            || self.coefficients.len() != self.selected_order + 1
            || self.edge_count > spec.maximum_edges
            || self.candidate_pair_evaluations > spec.maximum_candidate_pairs
            || self.matrix_vector_work > spec.maximum_matrix_vector_work
            || self.working_bytes > spec.maximum_working_bytes
            || self.matrix_vector_products != self.selected_order
            || self.graph_digest.len() != 64
            || self
                .graph_digest
                .bytes()
                .any(|byte| !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            || self
                .coefficients
                .iter()
                .chain(&self.filtered_signal)
                .chain([
                    &self.spectral_upper_bound,
                    &self.estimated_tail_bound,
                    &self.verified_grid_error,
                    &self.signal_l2_norm,
                    &self.diagnostic_l2_error_bound,
                ])
                .any(|value| !value.is_finite())
        {
            return Err(GraphError::Invalid(
                "sparse radius heat result does not match its typed specification".into(),
            ));
        }
        Ok(())
    }
}

pub fn graph_sparse_radius_heat_workflow(
    mut spec: GraphSparseRadiusHeatSpec,
) -> Result<GraphSparseRadiusHeatResult, GraphError> {
    validate(&spec)?;
    spec.nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let mut ids = BTreeSet::new();
    let mut cells = BTreeMap::<(i64, i64), Vec<usize>>::new();
    for (index, node) in spec.nodes.iter().enumerate() {
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
                "sparse heat nodes require unique exact IDs and finite coordinates/signals".into(),
            ));
        }
        let key = cell_key(node.coordinates_um, spec.radius_um)?;
        cells.entry(key).or_default().push(index);
    }

    let radius_squared = spec.radius_um * spec.radius_um;
    let mut candidates = 0_u64;
    let mut edges = Vec::<(usize, usize)>::new();
    let mut degrees = vec![0_u32; spec.nodes.len()];
    for (left, node) in spec.nodes.iter().enumerate() {
        let (cell_x, cell_y) = cell_key(node.coordinates_um, spec.radius_um)?;
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
                    if candidates > spec.maximum_candidate_pairs {
                        return Err(GraphError::Invalid(
                            "candidate pair count exceeds caller maximum".into(),
                        ));
                    }
                    let right_node = &spec.nodes[right];
                    let dx = node.coordinates_um[0] - right_node.coordinates_um[0];
                    let dy = node.coordinates_um[1] - right_node.coordinates_um[1];
                    if dx.mul_add(dx, dy * dy) <= radius_squared {
                        if edges.len() as u64 >= spec.maximum_edges {
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
    let maximum_degree = degrees.iter().copied().max().unwrap_or(0);
    if maximum_degree == 0 {
        return Err(GraphError::Invalid(
            "sparse radius graph must contain at least one edge".into(),
        ));
    }
    let spectral_upper_bound = 2.0 * f64::from(maximum_degree);
    let reference_order = (spec.maximum_order + 32).clamp(64, 512);
    let reference = heat_chebyshev_coefficients(spec.time, spectral_upper_bound, reference_order);
    let mut selected = None;
    for order in 1..=spec.maximum_order {
        let coefficients = reference[..=order].to_vec();
        let estimated_tail_bound = reference[(order + 1)..]
            .iter()
            .map(|value| value.abs())
            .sum::<f64>();
        let verified_grid_error =
            heat_grid_error(spec.time, spectral_upper_bound, &coefficients, 4_097);
        if estimated_tail_bound <= spec.tolerance && verified_grid_error <= spec.tolerance {
            selected = Some((
                order,
                coefficients,
                estimated_tail_bound,
                verified_grid_error,
            ));
            break;
        }
    }
    let (selected_order, coefficients, estimated_tail_bound, verified_grid_error) = selected
        .ok_or_else(|| {
            GraphError::Invalid(
                "requested sparse heat tolerance was not achieved by maximum order".into(),
            )
        })?;
    let matrix_vector_work = (selected_order as u64)
        .checked_mul(
            (spec.nodes.len() as u64)
                .checked_add(
                    (edges.len() as u64)
                        .checked_mul(2)
                        .ok_or_else(|| GraphError::Invalid("matvec work overflow".into()))?,
                )
                .ok_or_else(|| GraphError::Invalid("matvec work overflow".into()))?,
        )
        .ok_or_else(|| GraphError::Invalid("matvec work overflow".into()))?;
    if matrix_vector_work > spec.maximum_matrix_vector_work {
        return Err(GraphError::Invalid(
            "matrix-vector work exceeds caller maximum".into(),
        ));
    }
    let working_bytes = conservative_working_bytes(
        spec.nodes.len(),
        edges.len(),
        cells.len(),
        coefficients.len(),
    )?;
    if working_bytes > spec.maximum_working_bytes {
        return Err(GraphError::Invalid(
            "sparse heat working bytes exceed caller maximum".into(),
        ));
    }
    let signal = spec
        .nodes
        .iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    let filtered_signal = sparse_chebyshev_apply(
        &degrees,
        &edges,
        spectral_upper_bound,
        &signal,
        &coefficients,
    )?;
    let signal_l2_norm = signal.iter().map(|value| value * value).sum::<f64>().sqrt();
    let diagnostic_l2_error_bound = signal_l2_norm * (estimated_tail_bound + verified_grid_error);
    if !signal_l2_norm.is_finite() || !diagnostic_l2_error_bound.is_finite() {
        return Err(GraphError::Numerical(
            "sparse heat error diagnostic is non-finite".into(),
        ));
    }
    Ok(GraphSparseRadiusHeatResult {
        format: "marklab.graph_sparse_radius_heat".into(),
        version: 1,
        graph_digest: graph_digest(&spec.nodes, &edges, spec.radius_um),
        graph_rule: "uniform_cell_exact_physical_radius".into(),
        laplacian_kind: "combinatorial_binary_sparse".into(),
        node_count: spec.nodes.len(),
        edge_count: edges.len() as u64,
        isolated_node_count: degrees.iter().filter(|degree| **degree == 0).count(),
        radius_um: spec.radius_um,
        time: spec.time,
        spectral_upper_bound,
        selected_order,
        coefficients,
        estimated_tail_bound,
        verified_grid_error,
        tolerance: spec.tolerance,
        filtered_signal,
        signal_l2_norm,
        diagnostic_l2_error_bound,
        candidate_pair_evaluations: candidates,
        matrix_vector_products: selected_order,
        matrix_vector_work,
        working_bytes,
        claim_status: "experimental_sparse_graph_signal".into(),
    })
}

fn validate(spec: &GraphSparseRadiusHeatSpec) -> Result<(), GraphError> {
    if spec.nodes.len() < 2
        || spec.maximum_nodes < 2
        || spec.nodes.len() > spec.maximum_nodes
        || !spec.radius_um.is_finite()
        || spec.radius_um <= 0.0
        || !(spec.radius_um * spec.radius_um).is_finite()
        || !spec.time.is_finite()
        || spec.time <= 0.0
        || !spec.tolerance.is_finite()
        || !(0.0..1.0).contains(&spec.tolerance)
        || !(1..=256).contains(&spec.maximum_order)
        || spec.maximum_candidate_pairs == 0
        || spec.maximum_edges == 0
        || spec.maximum_matrix_vector_work == 0
        || spec.maximum_working_bytes == 0
    {
        return Err(GraphError::Invalid(
            "sparse radius heat controls or resource limits are invalid".into(),
        ));
    }
    Ok(())
}

fn cell_key(coordinates: [f64; 2], radius: f64) -> Result<(i64, i64), GraphError> {
    let scaled = [coordinates[0] / radius, coordinates[1] / radius];
    if scaled
        .iter()
        .any(|value| !value.is_finite() || *value < i64::MIN as f64 || *value > i64::MAX as f64)
    {
        return Err(GraphError::Invalid(
            "sparse heat coordinate cannot be represented by the radius grid".into(),
        ));
    }
    Ok((scaled[0].floor() as i64, scaled[1].floor() as i64))
}

fn conservative_working_bytes(
    nodes: usize,
    edges: usize,
    cells: usize,
    coefficients: usize,
) -> Result<u64, GraphError> {
    (nodes as u64)
        .checked_mul(320)
        .and_then(|value| value.checked_add((edges as u64).checked_mul(64)?))
        .and_then(|value| value.checked_add((cells as u64).checked_mul(128)?))
        .and_then(|value| value.checked_add((coefficients as u64).checked_mul(8)?))
        .ok_or_else(|| GraphError::Invalid("sparse heat working byte count overflow".into()))
}

fn sparse_chebyshev_apply(
    degrees: &[u32],
    edges: &[(usize, usize)],
    spectral_upper_bound: f64,
    input: &[f64],
    coefficients: &[f64],
) -> Result<Vec<f64>, GraphError> {
    let mut previous = input.to_vec();
    let mut output = previous
        .iter()
        .map(|value| 0.5 * coefficients[0] * value)
        .collect::<Vec<_>>();
    if coefficients.len() == 1 {
        return Ok(output);
    }
    let mut current = scaled_laplacian_product(degrees, edges, spectral_upper_bound, input);
    for (target, value) in output.iter_mut().zip(&current) {
        *target += coefficients[1] * value;
    }
    for &coefficient in coefficients.iter().skip(2) {
        let product = scaled_laplacian_product(degrees, edges, spectral_upper_bound, &current);
        let next = product
            .iter()
            .zip(&previous)
            .map(|(product, previous)| 2.0 * product - previous)
            .collect::<Vec<_>>();
        for (target, value) in output.iter_mut().zip(&next) {
            *target += coefficient * value;
        }
        previous = current;
        current = next;
    }
    if output.iter().any(|value| !value.is_finite()) {
        return Err(GraphError::Numerical(
            "sparse Chebyshev recurrence produced non-finite output".into(),
        ));
    }
    Ok(output)
}

fn scaled_laplacian_product(
    degrees: &[u32],
    edges: &[(usize, usize)],
    spectral_upper_bound: f64,
    input: &[f64],
) -> Vec<f64> {
    let scale = 2.0 / spectral_upper_bound;
    let mut output = degrees
        .iter()
        .zip(input)
        .map(|(degree, value)| (scale * f64::from(*degree) - 1.0) * value)
        .collect::<Vec<_>>();
    for &(left, right) in edges {
        output[left] -= scale * input[right];
        output[right] -= scale * input[left];
    }
    output
}

fn graph_digest(nodes: &[GraphNodeInput], edges: &[(usize, usize)], radius: f64) -> String {
    let mut digest = Sha256::new();
    digest.update(b"marklab-sparse-radius-graph-v1\0");
    digest.update(radius.to_bits().to_be_bytes());
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

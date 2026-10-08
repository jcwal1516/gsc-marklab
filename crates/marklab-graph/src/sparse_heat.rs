use serde::{Deserialize, Serialize};

use crate::{
    heat_chebyshev_coefficients, heat_grid_error,
    sparse_radius_graph::{build_sparse_radius_graph, graph_digest, laplacian_product},
    GraphError, GraphNodeInput,
};

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
    let graph = build_sparse_radius_graph(
        std::mem::take(&mut spec.nodes),
        spec.radius_um,
        spec.maximum_nodes,
        spec.maximum_candidate_pairs,
        spec.maximum_edges,
    )?;
    spec.nodes = graph.nodes;
    let edges = graph.edges;
    let degrees = graph.degrees;
    let candidates = graph.candidate_pair_evaluations;
    let maximum_degree = degrees.iter().copied().max().unwrap_or(0);
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
        graph.cell_count,
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
    laplacian_product(degrees, edges, input)
        .into_iter()
        .zip(input)
        .map(|(laplacian, value)| scale * laplacian - value)
        .collect()
}

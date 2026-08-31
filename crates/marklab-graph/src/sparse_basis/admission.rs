use super::model::GraphSparseRadiusBasisSpec;
use crate::{sparse_radius_graph::SparseRadiusGraph, GraphError};

pub(super) const MAXIMUM_MODES: usize = 128;
pub(super) const MAXIMUM_ITERATIONS: usize = 512;

pub(super) struct ComponentGraph {
    pub(super) global_nodes: Vec<usize>,
    pub(super) degrees: Vec<u32>,
    pub(super) edges: Vec<(usize, usize)>,
}

pub(super) struct WorkPlan {
    pub(super) matrix_vector_products: u64,
    pub(super) matrix_vector_work: u64,
    pub(super) orthogonalization_base_work: u64,
    pub(super) maximum_candidate_modes: usize,
    pub(super) working_bytes: u64,
    pub(super) retained_bytes: u64,
}

pub(super) fn validate_spec(spec: &GraphSparseRadiusBasisSpec) -> Result<(), GraphError> {
    if !(2..=MAXIMUM_MODES).contains(&spec.mode_count)
        || spec.mode_count > spec.nodes.len()
        || !(1..=MAXIMUM_ITERATIONS).contains(&spec.maximum_iterations)
        || !spec.residual_tolerance.is_finite()
        || !(1e-12..=0.1).contains(&spec.residual_tolerance)
        || spec.maximum_components == 0
        || spec.maximum_matrix_vector_work == 0
        || spec.maximum_orthogonalization_work == 0
        || spec.maximum_ritz_rotations == 0
        || spec.maximum_working_bytes == 0
        || spec.maximum_retained_bytes == 0
    {
        return Err(GraphError::Invalid(
            "sparse radius basis controls or resource limits are invalid".into(),
        ));
    }
    Ok(())
}

pub(super) fn component_graphs(
    graph: &SparseRadiusGraph,
) -> Result<Vec<ComponentGraph>, GraphError> {
    let mut adjacency = vec![Vec::new(); graph.nodes.len()];
    for &(left, right) in &graph.edges {
        adjacency[left].push(right);
        adjacency[right].push(left);
    }
    let mut seen = vec![false; graph.nodes.len()];
    let mut components = Vec::new();
    for start in 0..graph.nodes.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![start];
        let mut globals = Vec::new();
        while let Some(node) = stack.pop() {
            globals.push(node);
            for &neighbor in &adjacency[node] {
                if !seen[neighbor] {
                    seen[neighbor] = true;
                    stack.push(neighbor);
                }
            }
        }
        globals.sort_unstable();
        let mut local_index = vec![usize::MAX; graph.nodes.len()];
        for (local, &global) in globals.iter().enumerate() {
            local_index[global] = local;
        }
        let mut edges = Vec::new();
        for &global in &globals {
            for &neighbor in &adjacency[global] {
                if global < neighbor {
                    edges.push((local_index[global], local_index[neighbor]));
                }
            }
        }
        let mut degrees = vec![0_u32; globals.len()];
        for &(left, right) in &edges {
            degrees[left] = degrees[left]
                .checked_add(1)
                .ok_or_else(|| GraphError::Invalid("component degree overflow".into()))?;
            degrees[right] = degrees[right]
                .checked_add(1)
                .ok_or_else(|| GraphError::Invalid("component degree overflow".into()))?;
        }
        components.push(ComponentGraph {
            global_nodes: globals,
            degrees,
            edges,
        });
    }
    Ok(components)
}

pub(super) fn plan_work(
    spec: &GraphSparseRadiusBasisSpec,
    graph: &SparseRadiusGraph,
    components: &[ComponentGraph],
) -> Result<WorkPlan, GraphError> {
    let nonzero_needed = spec.mode_count - components.len();
    let mut matrix_vector_products = 0_u64;
    let mut matrix_vector_work = 0_u64;
    let mut orthogonalization_base_work = 0_u64;
    let mut maximum_candidate_modes = 0_usize;
    let mut maximum_component_workspace = 0_u64;
    for component in components {
        let nodes = component.global_nodes.len() as u64;
        let candidate_modes = nonzero_needed.min(component.global_nodes.len().saturating_sub(1));
        maximum_candidate_modes = maximum_candidate_modes.max(candidate_modes);
        if candidate_modes == 0 {
            continue;
        }
        let modes = candidate_modes as u64;
        let products = modes
            .checked_mul((spec.maximum_iterations + 1) as u64)
            .ok_or_else(|| GraphError::Invalid("matrix-vector product count overflow".into()))?;
        let per_product = nodes
            .checked_add(
                (component.edges.len() as u64)
                    .checked_mul(2)
                    .ok_or_else(|| GraphError::Invalid("matrix-vector work overflow".into()))?,
            )
            .ok_or_else(|| GraphError::Invalid("matrix-vector work overflow".into()))?;
        matrix_vector_products = matrix_vector_products
            .checked_add(products)
            .ok_or_else(|| GraphError::Invalid("matrix-vector product count overflow".into()))?;
        matrix_vector_work = matrix_vector_work
            .checked_add(
                products
                    .checked_mul(per_product)
                    .ok_or_else(|| GraphError::Invalid("matrix-vector work overflow".into()))?,
            )
            .ok_or_else(|| GraphError::Invalid("matrix-vector work overflow".into()))?;
        let dense_factor = (2 * (spec.maximum_iterations + 1) + 7) as u64;
        orthogonalization_base_work = orthogonalization_base_work
            .checked_add(
                dense_factor
                    .checked_mul(nodes)
                    .and_then(|value| value.checked_mul(modes))
                    .and_then(|value| value.checked_mul(modes))
                    .ok_or_else(|| GraphError::Invalid("orthogonalization work overflow".into()))?,
            )
            .ok_or_else(|| GraphError::Invalid("orthogonalization work overflow".into()))?;
        maximum_component_workspace = maximum_component_workspace.max(
            nodes
                .checked_mul(modes)
                .and_then(|value| value.checked_mul(24))
                .and_then(|value| value.checked_add(modes.checked_mul(modes)?.checked_mul(32)?))
                .ok_or_else(|| GraphError::Invalid("basis workspace overflow".into()))?,
        );
    }
    let retained_bytes = (graph.nodes.len() as u64)
        .checked_mul(32)
        .and_then(|value| value.checked_add((graph.nodes.len() as u64).checked_mul(8)?))
        .and_then(|value| {
            value.checked_add(
                ((nonzero_needed + 1) as u64)
                    .checked_mul(graph.nodes.len() as u64)?
                    .checked_mul(24)?,
            )
        })
        .and_then(|value| value.checked_add((spec.mode_count as u64).checked_mul(256)?))
        .and_then(|value| {
            value.checked_add(
                graph
                    .nodes
                    .iter()
                    .map(|node| node.id.len() as u64)
                    .sum::<u64>(),
            )
        })
        .ok_or_else(|| GraphError::Invalid("retained byte count overflow".into()))?;
    let candidate_storage = (nonzero_needed as u64)
        .checked_mul(graph.nodes.len() as u64)
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| GraphError::Invalid("candidate storage overflow".into()))?;
    let working_bytes = (graph.nodes.len() as u64)
        .checked_mul(384)
        .and_then(|value| value.checked_add((graph.edges.len() as u64).checked_mul(96)?))
        .and_then(|value| value.checked_add(maximum_component_workspace))
        .and_then(|value| value.checked_add(candidate_storage))
        .and_then(|value| value.checked_add(retained_bytes))
        .ok_or_else(|| GraphError::Invalid("basis working byte count overflow".into()))?;
    Ok(WorkPlan {
        matrix_vector_products,
        matrix_vector_work,
        orthogonalization_base_work,
        maximum_candidate_modes,
        working_bytes,
        retained_bytes,
    })
}

use serde::{Deserialize, Serialize};

use crate::{
    sparse_radius_graph::{
        build_sparse_radius_graph, graph_digest, laplacian_product, SparseRadiusGraph,
    },
    symmetric_eigendecomposition_with_limit, GraphError, GraphNodeInput,
};

const MAXIMUM_MODES: usize = 128;
const MAXIMUM_ITERATIONS: usize = 512;
const SUBSPACE_STEP: f64 = 0.99;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusBasisSpec {
    pub nodes: Vec<GraphNodeInput>,
    pub radius_um: f64,
    pub mode_count: usize,
    pub maximum_iterations: usize,
    pub residual_tolerance: f64,
    pub maximum_nodes: usize,
    pub maximum_candidate_pairs: u64,
    pub maximum_edges: u64,
    pub maximum_components: usize,
    pub maximum_matrix_vector_work: u64,
    pub maximum_orthogonalization_work: u64,
    pub maximum_ritz_rotations: usize,
    pub maximum_working_bytes: u64,
    pub maximum_retained_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseRadiusBasisMode {
    pub mode_index: usize,
    pub component_index: usize,
    pub eigenvalue: f64,
    pub residual_l2: f64,
    pub component_zero_mode: bool,
    pub values_by_component_node: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseRadiusBasisComponent {
    pub component_index: usize,
    pub node_indices: Vec<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusBasisResult {
    pub format: String,
    pub version: u32,
    pub graph_digest: String,
    pub graph_rule: String,
    pub laplacian_kind: String,
    pub basis_method: String,
    pub component_policy: String,
    pub signal_policy: String,
    pub node_ids: Vec<String>,
    pub node_count: usize,
    pub edge_count: u64,
    pub isolated_node_count: usize,
    pub component_count: usize,
    pub component_sizes: Vec<usize>,
    pub components: Vec<SparseRadiusBasisComponent>,
    pub radius_um: f64,
    pub requested_mode_count: usize,
    pub returned_mode_count: usize,
    pub iterations: usize,
    pub residual_tolerance: f64,
    pub modes: Vec<SparseRadiusBasisMode>,
    pub maximum_residual_l2: f64,
    pub orthogonality_max_abs_error: f64,
    pub candidate_pair_evaluations: u64,
    pub matrix_vector_products: u64,
    pub matrix_vector_work: u64,
    pub orthogonalization_work: u64,
    pub ritz_rotations: usize,
    pub working_bytes: u64,
    pub retained_bytes: u64,
    pub claim_status: String,
}

impl GraphSparseRadiusBasisResult {
    pub fn validate_for_spec(&self, spec: &GraphSparseRadiusBasisSpec) -> Result<(), GraphError> {
        if self.component_count == 0
            || self.component_count > spec.mode_count
            || self.component_count > spec.nodes.len()
            || self.modes.len() != spec.mode_count
            || self.component_sizes.len() != self.component_count
            || self.components.len() != self.component_count
        {
            return Err(GraphError::Invalid(
                "sparse radius basis structure is outside the typed request bounds".into(),
            ));
        }
        let mut node_ids = spec
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        node_ids.sort();
        let mut component_nodes = self
            .components
            .iter()
            .flat_map(|component| component.node_indices.iter().copied())
            .collect::<Vec<_>>();
        component_nodes.sort_unstable();
        let derived_maximum_residual_l2 = self
            .modes
            .iter()
            .map(|mode| mode.residual_l2)
            .fold(0.0_f64, f64::max);
        let derived_orthogonality_error = orthogonality_error(&self.modes);
        let component_indices_valid = self
            .modes
            .iter()
            .all(|mode| mode.component_index < self.component_count);
        let mut zero_modes_by_component = vec![0_usize; self.component_count];
        if component_indices_valid {
            for mode in self.modes.iter().filter(|mode| mode.component_zero_mode) {
                zero_modes_by_component[mode.component_index] += 1;
            }
        }
        if self.format != "marklab.graph_sparse_radius_basis"
            || self.version != 1
            || self.graph_rule != "uniform_cell_exact_physical_radius"
            || self.laplacian_kind != "combinatorial_binary_sparse"
            || self.basis_method
                != "component_zero_modes_plus_shifted_laplacian_subspace_iteration_ritz"
            || self.component_policy
                != "retain_complete_component_zero_eigenspace_then_global_low_modes"
            || self.signal_policy != "ignored_basis_depends_only_on_geometry"
            || self.claim_status != "experimental_sparse_low_frequency_basis"
            || self.node_ids != node_ids
            || self.node_count != spec.nodes.len()
            || self.radius_um.to_bits() != spec.radius_um.to_bits()
            || self.requested_mode_count != spec.mode_count
            || self.returned_mode_count != spec.mode_count
            || self.iterations != spec.maximum_iterations
            || self.residual_tolerance.to_bits() != spec.residual_tolerance.to_bits()
            || self.modes.len() != spec.mode_count
            || self.component_sizes.len() != self.component_count
            || self.components.len() != self.component_count
            || component_nodes != (0..self.node_count).collect::<Vec<_>>()
            || self.component_sizes.iter().sum::<usize>() != self.node_count
            || self.component_count > spec.maximum_components
            || self
                .modes
                .iter()
                .filter(|mode| mode.component_zero_mode)
                .count()
                != self.component_count
            || !component_indices_valid
            || zero_modes_by_component.iter().any(|count| *count != 1)
            || self
                .modes
                .iter()
                .take(self.component_count)
                .any(|mode| !mode.component_zero_mode)
            || self
                .modes
                .iter()
                .skip(self.component_count)
                .any(|mode| mode.component_zero_mode)
            || self
                .modes
                .windows(2)
                .any(|pair| pair[0].eigenvalue.total_cmp(&pair[1].eigenvalue).is_gt())
            || self.candidate_pair_evaluations > spec.maximum_candidate_pairs
            || self.edge_count > spec.maximum_edges
            || self.matrix_vector_work > spec.maximum_matrix_vector_work
            || self.orthogonalization_work > spec.maximum_orthogonalization_work
            || self.ritz_rotations > spec.maximum_ritz_rotations
            || self.working_bytes > spec.maximum_working_bytes
            || self.retained_bytes > spec.maximum_retained_bytes
            || self.maximum_residual_l2 > spec.residual_tolerance
            || self.orthogonality_max_abs_error > spec.residual_tolerance.max(1e-10)
            || self.maximum_residual_l2.to_bits() != derived_maximum_residual_l2.to_bits()
            || self.orthogonality_max_abs_error.to_bits() != derived_orthogonality_error.to_bits()
            || self.graph_digest.len() != 64
            || self.modes.iter().enumerate().any(|(index, mode)| {
                mode.mode_index != index
                    || mode.component_index >= self.component_count
                    || mode.values_by_component_node.len()
                        != self.component_sizes[mode.component_index]
                    || !mode.eigenvalue.is_finite()
                    || mode.eigenvalue < 0.0
                    || (mode.component_zero_mode
                        && (mode.eigenvalue.to_bits() != 0.0_f64.to_bits()
                            || mode.residual_l2.to_bits() != 0.0_f64.to_bits()))
                    || !mode.residual_l2.is_finite()
                    || mode.residual_l2 > spec.residual_tolerance
                    || mode
                        .values_by_component_node
                        .iter()
                        .any(|value| !value.is_finite())
            })
            || self
                .components
                .iter()
                .enumerate()
                .any(|(index, component)| {
                    component.component_index != index
                        || component.node_indices.len() != self.component_sizes[index]
                        || component
                            .node_indices
                            .iter()
                            .any(|node| *node >= self.node_count)
                })
        {
            return Err(GraphError::Invalid(
                "sparse radius basis result does not match its typed specification".into(),
            ));
        }
        Ok(())
    }
}

struct ComponentGraph {
    global_nodes: Vec<usize>,
    degrees: Vec<u32>,
    edges: Vec<(usize, usize)>,
}

struct CandidateMode {
    component_index: usize,
    eigenvalue: f64,
    residual_l2: f64,
    zero_mode: bool,
    local_values: Vec<f64>,
}

struct WorkPlan {
    matrix_vector_products: u64,
    matrix_vector_work: u64,
    orthogonalization_base_work: u64,
    maximum_candidate_modes: usize,
    working_bytes: u64,
    retained_bytes: u64,
}

pub fn graph_sparse_radius_basis_workflow(
    mut spec: GraphSparseRadiusBasisSpec,
) -> Result<GraphSparseRadiusBasisResult, GraphError> {
    validate_spec(&spec)?;
    let graph = build_sparse_radius_graph(
        std::mem::take(&mut spec.nodes),
        spec.radius_um,
        spec.maximum_nodes,
        spec.maximum_candidate_pairs,
        spec.maximum_edges,
    )?;
    let components = component_graphs(&graph)?;
    if components.len() > spec.maximum_components {
        return Err(GraphError::Invalid(
            "component count exceeds caller maximum".into(),
        ));
    }
    if spec.mode_count < components.len() {
        return Err(GraphError::Invalid(
            "mode count must retain the complete component-zero eigenspace".into(),
        ));
    }
    let plan = plan_work(&spec, &graph, &components)?;
    if plan.matrix_vector_work > spec.maximum_matrix_vector_work
        || plan
            .orthogonalization_base_work
            .checked_add(
                (spec.maximum_ritz_rotations as u64)
                    .checked_mul(8)
                    .and_then(|value| value.checked_mul(plan.maximum_candidate_modes as u64))
                    .ok_or_else(|| GraphError::Invalid("Ritz work overflow".into()))?,
            )
            .ok_or_else(|| GraphError::Invalid("orthogonalization work overflow".into()))?
            > spec.maximum_orthogonalization_work
        || plan.working_bytes > spec.maximum_working_bytes
        || plan.retained_bytes > spec.maximum_retained_bytes
    {
        return Err(GraphError::Invalid(
            "sparse basis work, memory, or retained output exceeds caller maximum".into(),
        ));
    }

    let nonzero_needed = spec.mode_count - components.len();
    let mut candidates = Vec::new();
    for (component_index, component) in components.iter().enumerate() {
        let normalization = (component.global_nodes.len() as f64).sqrt().recip();
        candidates.push(CandidateMode {
            component_index,
            eigenvalue: 0.0,
            residual_l2: 0.0,
            zero_mode: true,
            local_values: vec![normalization; component.global_nodes.len()],
        });
    }
    let mut rotations = 0_usize;
    for (component_index, component) in components.iter().enumerate() {
        let candidate_count = nonzero_needed.min(component.global_nodes.len().saturating_sub(1));
        if candidate_count == 0 {
            continue;
        }
        let remaining_rotations = spec
            .maximum_ritz_rotations
            .checked_sub(rotations)
            .ok_or_else(|| GraphError::Invalid("Ritz rotation count overflow".into()))?;
        let (mut component_modes, used_rotations) = approximate_component_modes(
            component_index,
            component,
            candidate_count,
            spec.maximum_iterations,
            remaining_rotations,
        )?;
        rotations = rotations
            .checked_add(used_rotations)
            .ok_or_else(|| GraphError::Invalid("Ritz rotation count overflow".into()))?;
        candidates.append(&mut component_modes);
    }
    candidates.sort_by(|left, right| {
        left.eigenvalue
            .total_cmp(&right.eigenvalue)
            .then(right.zero_mode.cmp(&left.zero_mode))
            .then(left.component_index.cmp(&right.component_index))
    });
    candidates.truncate(spec.mode_count);
    if candidates.len() != spec.mode_count {
        return Err(GraphError::Numerical(
            "sparse basis did not produce the requested complete mode count".into(),
        ));
    }

    let mut modes = Vec::with_capacity(spec.mode_count);
    for (mode_index, candidate) in candidates.into_iter().enumerate() {
        if !candidate.zero_mode && candidate.residual_l2 > spec.residual_tolerance {
            return Err(GraphError::Numerical(format!(
                "sparse basis residual {} exceeds tolerance {}",
                candidate.residual_l2, spec.residual_tolerance
            )));
        }
        let mut values = candidate.local_values;
        orient_mode(&mut values);
        modes.push(SparseRadiusBasisMode {
            mode_index,
            component_index: candidate.component_index,
            eigenvalue: candidate.eigenvalue,
            residual_l2: candidate.residual_l2,
            component_zero_mode: candidate.zero_mode,
            values_by_component_node: values,
        });
    }
    let maximum_residual_l2 = modes
        .iter()
        .map(|mode| mode.residual_l2)
        .fold(0.0_f64, f64::max);
    let orthogonality_max_abs_error = orthogonality_error(&modes);
    if !maximum_residual_l2.is_finite()
        || !orthogonality_max_abs_error.is_finite()
        || orthogonality_max_abs_error > spec.residual_tolerance.max(1e-10)
    {
        return Err(GraphError::Numerical(
            "sparse basis orthogonality or residual diagnostic failed".into(),
        ));
    }
    let rotation_work = (rotations as u64)
        .checked_mul(8)
        .and_then(|value| value.checked_mul(plan.maximum_candidate_modes as u64))
        .ok_or_else(|| GraphError::Invalid("Ritz work overflow".into()))?;
    let orthogonalization_work = plan
        .orthogonalization_base_work
        .checked_add(rotation_work)
        .ok_or_else(|| GraphError::Invalid("orthogonalization work overflow".into()))?;
    let graph_hash = graph_digest(&graph.nodes, &graph.edges, spec.radius_um);
    Ok(GraphSparseRadiusBasisResult {
        format: "marklab.graph_sparse_radius_basis".into(),
        version: 1,
        graph_digest: graph_hash,
        graph_rule: "uniform_cell_exact_physical_radius".into(),
        laplacian_kind: "combinatorial_binary_sparse".into(),
        basis_method: "component_zero_modes_plus_shifted_laplacian_subspace_iteration_ritz".into(),
        component_policy: "retain_complete_component_zero_eigenspace_then_global_low_modes".into(),
        signal_policy: "ignored_basis_depends_only_on_geometry".into(),
        node_ids: graph.nodes.iter().map(|node| node.id.clone()).collect(),
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len() as u64,
        isolated_node_count: graph.degrees.iter().filter(|degree| **degree == 0).count(),
        component_count: components.len(),
        component_sizes: components
            .iter()
            .map(|component| component.global_nodes.len())
            .collect(),
        components: components
            .iter()
            .enumerate()
            .map(|(component_index, component)| SparseRadiusBasisComponent {
                component_index,
                node_indices: component.global_nodes.clone(),
            })
            .collect(),
        radius_um: spec.radius_um,
        requested_mode_count: spec.mode_count,
        returned_mode_count: modes.len(),
        iterations: spec.maximum_iterations,
        residual_tolerance: spec.residual_tolerance,
        modes,
        maximum_residual_l2,
        orthogonality_max_abs_error,
        candidate_pair_evaluations: graph.candidate_pair_evaluations,
        matrix_vector_products: plan.matrix_vector_products,
        matrix_vector_work: plan.matrix_vector_work,
        orthogonalization_work,
        ritz_rotations: rotations,
        working_bytes: plan.working_bytes,
        retained_bytes: plan.retained_bytes,
        claim_status: "experimental_sparse_low_frequency_basis".into(),
    })
}

fn validate_spec(spec: &GraphSparseRadiusBasisSpec) -> Result<(), GraphError> {
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

fn component_graphs(graph: &SparseRadiusGraph) -> Result<Vec<ComponentGraph>, GraphError> {
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

fn plan_work(
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

fn approximate_component_modes(
    component_index: usize,
    component: &ComponentGraph,
    mode_count: usize,
    iterations: usize,
    maximum_rotations: usize,
) -> Result<(Vec<CandidateMode>, usize), GraphError> {
    let size = component.global_nodes.len();
    let mut basis = (0..mode_count)
        .map(|column| {
            let mut values = vec![-1.0 / size as f64; size];
            values[column] += 1.0;
            values
        })
        .collect::<Vec<_>>();
    orthonormalize_zero_sum(&mut basis)?;
    let maximum_degree = component.degrees.iter().copied().max().unwrap_or(0);
    let spectral_upper_bound = 2.0 * f64::from(maximum_degree);
    let scale = SUBSPACE_STEP / spectral_upper_bound;
    for _ in 0..iterations {
        for vector in &mut basis {
            let laplacian = laplacian_product(&component.degrees, &component.edges, vector);
            for (value, applied) in vector.iter_mut().zip(laplacian) {
                *value -= scale * applied;
            }
        }
        orthonormalize_zero_sum(&mut basis)?;
    }
    let applied = basis
        .iter()
        .map(|vector| laplacian_product(&component.degrees, &component.edges, vector))
        .collect::<Vec<_>>();
    let mut projected = vec![vec![0.0; mode_count]; mode_count];
    for left in 0..mode_count {
        for right in left..mode_count {
            let value = dot(&basis[left], &applied[right]);
            let reverse = dot(&basis[right], &applied[left]);
            projected[left][right] = 0.5 * (value + reverse);
            projected[right][left] = projected[left][right];
        }
    }
    let (eigenvalues, coefficients, rotations) = if mode_count == 1 {
        (vec![projected[0][0]], vec![vec![1.0]], 0)
    } else {
        let decomposition = symmetric_eigendecomposition_with_limit(&projected, maximum_rotations)?;
        (
            decomposition.values,
            decomposition.vectors,
            decomposition.rotations,
        )
    };
    let mut modes = Vec::with_capacity(mode_count);
    for (eigenvalue, coefficient) in eigenvalues.into_iter().zip(coefficients) {
        let mut values = linear_combination(&basis, &coefficient);
        let applied_values = linear_combination(&applied, &coefficient);
        let residual_l2 = applied_values
            .iter()
            .zip(&values)
            .map(|(applied, value)| (applied - eigenvalue * value).powi(2))
            .sum::<f64>()
            .sqrt();
        if !eigenvalue.is_finite()
            || eigenvalue < -1e-10
            || !residual_l2.is_finite()
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(GraphError::Numerical(
                "sparse basis Ritz result is non-finite or negative".into(),
            ));
        }
        orient_mode(&mut values);
        modes.push(CandidateMode {
            component_index,
            eigenvalue: eigenvalue.max(0.0),
            residual_l2,
            zero_mode: false,
            local_values: values,
        });
    }
    Ok((modes, rotations))
}

fn orthonormalize_zero_sum(columns: &mut [Vec<f64>]) -> Result<(), GraphError> {
    for column in 0..columns.len() {
        subtract_mean(&mut columns[column]);
        for _ in 0..2 {
            let (previous, current) = columns.split_at_mut(column);
            let current = &mut current[0];
            for vector in previous {
                let projection = dot(current, vector);
                for (value, basis) in current.iter_mut().zip(vector) {
                    *value -= projection * *basis;
                }
            }
        }
        subtract_mean(&mut columns[column]);
        let norm = dot(&columns[column], &columns[column]).sqrt();
        if !norm.is_finite() || norm <= 1e-14 {
            return Err(GraphError::Numerical(
                "sparse basis subspace lost numerical rank".into(),
            ));
        }
        for value in &mut columns[column] {
            *value /= norm;
        }
    }
    Ok(())
}

fn subtract_mean(values: &mut [f64]) {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    for value in values {
        *value -= mean;
    }
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn linear_combination(columns: &[Vec<f64>], coefficients: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0; columns[0].len()];
    for (column, coefficient) in columns.iter().zip(coefficients) {
        for (value, basis) in result.iter_mut().zip(column) {
            *value += coefficient * basis;
        }
    }
    result
}

fn orient_mode(values: &mut [f64]) {
    if values
        .iter()
        .find(|value| value.abs() > 1e-12)
        .is_some_and(|value| *value < 0.0)
    {
        for value in values {
            *value = -*value;
        }
    }
}

fn orthogonality_error(modes: &[SparseRadiusBasisMode]) -> f64 {
    let mut maximum = 0.0_f64;
    for (left, left_mode) in modes.iter().enumerate() {
        for (right, right_mode) in modes.iter().enumerate().skip(left) {
            let expected = f64::from(left == right);
            let observed = if left_mode.component_index == right_mode.component_index {
                dot(
                    &left_mode.values_by_component_node,
                    &right_mode.values_by_component_node,
                )
            } else {
                0.0
            };
            maximum = maximum.max((observed - expected).abs());
        }
    }
    maximum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_spec() -> GraphSparseRadiusBasisSpec {
        GraphSparseRadiusBasisSpec {
            nodes: (0..5)
                .map(|index| GraphNodeInput {
                    id: format!("cell-{index}"),
                    coordinates_um: [index as f64, 0.0],
                    signal: index as f64,
                })
                .collect(),
            radius_um: 1.1,
            mode_count: 3,
            maximum_iterations: 96,
            residual_tolerance: 1e-9,
            maximum_nodes: 5,
            maximum_candidate_pairs: 10,
            maximum_edges: 4,
            maximum_components: 1,
            maximum_matrix_vector_work: 10_000,
            maximum_orthogonalization_work: 100_000,
            maximum_ritz_rotations: 1_000,
            maximum_working_bytes: 64_000,
            maximum_retained_bytes: 64_000,
        }
    }

    #[test]
    fn disconnected_graph_retains_the_complete_zero_eigenspace() {
        let mut spec = path_spec();
        spec.nodes = vec![
            GraphNodeInput {
                id: "a".into(),
                coordinates_um: [0.0, 0.0],
                signal: 0.0,
            },
            GraphNodeInput {
                id: "b".into(),
                coordinates_um: [1.0, 0.0],
                signal: 0.0,
            },
            GraphNodeInput {
                id: "c".into(),
                coordinates_um: [10.0, 0.0],
                signal: 0.0,
            },
            GraphNodeInput {
                id: "d".into(),
                coordinates_um: [11.0, 0.0],
                signal: 0.0,
            },
        ];
        spec.mode_count = 2;
        spec.maximum_nodes = 4;
        spec.maximum_edges = 2;
        spec.maximum_components = 2;
        let result = graph_sparse_radius_basis_workflow(spec).expect("basis");
        assert_eq!(result.component_count, 2);
        assert_eq!(result.component_sizes, vec![2, 2]);
        assert!(result.modes.iter().all(|mode| mode.component_zero_mode));
        assert!(result.modes.iter().all(|mode| mode.eigenvalue == 0.0));
        assert_eq!(result.matrix_vector_products, 0);
    }

    #[test]
    fn matrix_vector_work_is_admitted_before_iteration() {
        let mut spec = path_spec();
        spec.maximum_iterations = 10;
        spec.residual_tolerance = 0.1;
        spec.maximum_matrix_vector_work = 285;
        let error = graph_sparse_radius_basis_workflow(spec).unwrap_err();
        assert!(error.to_string().contains("exceeds caller maximum"));
    }

    #[test]
    fn basis_ignores_signal_values_but_retains_geometry_identity() {
        let first = graph_sparse_radius_basis_workflow(path_spec()).expect("first");
        let mut changed = path_spec();
        for node in &mut changed.nodes {
            node.signal = 100.0 - node.signal;
        }
        let second = graph_sparse_radius_basis_workflow(changed).expect("second");
        assert_eq!(first.graph_digest, second.graph_digest);
        assert_eq!(first.modes.len(), second.modes.len());
        for (left, right) in first.modes.iter().zip(second.modes) {
            assert_eq!(left.eigenvalue.to_bits(), right.eigenvalue.to_bits());
            assert_eq!(
                left.values_by_component_node,
                right.values_by_component_node
            );
        }
    }

    #[test]
    fn typed_result_rejects_corrupted_derived_diagnostics_and_mode_order() {
        let spec = path_spec();
        let result = graph_sparse_radius_basis_workflow(spec.clone()).expect("basis");

        let mut corrupted_residual = result.clone();
        corrupted_residual.maximum_residual_l2 = 0.0;
        assert!(corrupted_residual.validate_for_spec(&spec).is_err());

        let mut corrupted_orthogonality = result.clone();
        corrupted_orthogonality.orthogonality_max_abs_error = 0.0;
        assert!(corrupted_orthogonality.validate_for_spec(&spec).is_err());

        let mut corrupted_order = result.clone();
        corrupted_order.modes.swap(1, 2);
        for (index, mode) in corrupted_order.modes.iter_mut().enumerate() {
            mode.mode_index = index;
        }
        assert!(corrupted_order.validate_for_spec(&spec).is_err());

        let mut oversized_component_count = result;
        oversized_component_count.component_count = usize::MAX;
        let validation =
            std::panic::catch_unwind(|| oversized_component_count.validate_for_spec(&spec));
        assert!(matches!(validation, Ok(Err(_))));
    }

    #[test]
    fn pathology_component_count_can_retain_eight_nonzero_modes_above_sixty_four_total() {
        let mut spec = path_spec();
        spec.nodes = (0..65)
            .flat_map(|component| {
                (0..2).map(move |within| GraphNodeInput {
                    id: format!("component-{component:02}-node-{within}"),
                    coordinates_um: [component as f64 * 10.0 + within as f64, 0.0],
                    signal: within as f64,
                })
            })
            .collect();
        spec.mode_count = 73;
        spec.maximum_iterations = 1;
        spec.maximum_nodes = 130;
        spec.maximum_candidate_pairs = 8_385;
        spec.maximum_edges = 65;
        spec.maximum_components = 65;
        spec.maximum_matrix_vector_work = 1_000;
        spec.maximum_orthogonalization_work = 10_000;
        spec.maximum_ritz_rotations = 1;
        spec.maximum_working_bytes = 1_000_000;
        spec.maximum_retained_bytes = 1_000_000;

        let result = graph_sparse_radius_basis_workflow(spec).expect("expanded bounded basis");
        assert_eq!(result.component_count, 65);
        assert_eq!(result.returned_mode_count, 73);
        assert_eq!(
            result
                .modes
                .iter()
                .filter(|mode| mode.component_zero_mode)
                .count(),
            65
        );
    }
}

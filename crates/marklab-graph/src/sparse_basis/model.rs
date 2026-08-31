use serde::{Deserialize, Serialize};

use super::numerical::orthogonality_error;
use crate::{GraphError, GraphNodeInput};

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

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    graph_sparse_radius_basis_workflow, GraphError, GraphNodeInput, GraphSparseRadiusBasisResult,
    GraphSparseRadiusBasisSpec,
};

const SUMMARY_BASE_BYTES: u64 = 4_096;
const SUMMARY_MODE_BYTES: u64 = 256;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusFourierEnergySpec {
    pub basis: GraphSparseRadiusBasisSpec,
    pub maximum_projection_work: u64,
    pub maximum_fourier_retained_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseRadiusFourierModeEnergy {
    pub mode_index: usize,
    pub component_index: usize,
    pub component_node_count: usize,
    pub eigenvalue: f64,
    pub coefficient: f64,
    pub energy: f64,
    pub component_zero_mode: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSparseRadiusFourierEnergyResult {
    pub format: String,
    pub version: u32,
    pub graph_digest: String,
    pub signal_digest: String,
    pub graph_rule: String,
    pub laplacian_kind: String,
    pub basis_method: String,
    pub component_policy: String,
    pub signal_policy: String,
    pub node_count: usize,
    pub edge_count: u64,
    pub isolated_node_count: usize,
    pub component_count: usize,
    pub component_sizes: Vec<usize>,
    pub radius_um: f64,
    pub requested_mode_count: usize,
    pub returned_mode_count: usize,
    pub basis_iterations: usize,
    pub basis_residual_tolerance: f64,
    pub basis_maximum_residual_l2: f64,
    pub basis_orthogonality_max_abs_error: f64,
    pub mode_energies: Vec<SparseRadiusFourierModeEnergy>,
    pub total_signal_energy: f64,
    pub component_zero_energy: f64,
    pub centered_signal_energy: f64,
    pub nonzero_low_frequency_energy: f64,
    pub captured_energy: f64,
    pub unresolved_centered_energy: f64,
    pub captured_total_energy_fraction: Option<f64>,
    pub captured_centered_energy_fraction: Option<f64>,
    pub nonzero_low_frequency_fraction_of_total: Option<f64>,
    pub total_signal_status: String,
    pub centered_signal_status: String,
    pub planned_projection_work: u64,
    pub projection_work: u64,
    pub estimated_fourier_retained_bytes: u64,
    pub basis_candidate_pair_evaluations: u64,
    pub basis_matrix_vector_work: u64,
    pub basis_orthogonalization_work: u64,
    pub claim_status: String,
}

struct EnergySummary {
    centered_signal_energy: f64,
    captured_energy: f64,
    unresolved_centered_energy: f64,
    captured_total_energy_fraction: Option<f64>,
    captured_centered_energy_fraction: Option<f64>,
    nonzero_low_frequency_fraction_of_total: Option<f64>,
    total_signal_status: &'static str,
    centered_signal_status: &'static str,
}

impl GraphSparseRadiusFourierEnergyResult {
    pub fn validate_for_spec(
        &self,
        spec: &GraphSparseRadiusFourierEnergySpec,
    ) -> Result<(), GraphError> {
        if self.mode_energies.len() != spec.basis.mode_count
            || self.component_count == 0
            || self.component_count > spec.basis.mode_count
            || self.component_count > spec.basis.nodes.len()
            || self.component_sizes.len() != self.component_count
        {
            return Err(GraphError::Invalid(
                "sparse Fourier result structure is outside the typed request bounds".into(),
            ));
        }
        let planned_projection_work = planned_projection_work(spec)?;
        let estimated_retained_bytes = estimated_retained_bytes(spec.basis.mode_count)?;
        let mut sorted_nodes = spec.basis.nodes.clone();
        sorted_nodes.sort_by(|left, right| left.id.cmp(&right.id));
        let total_signal_energy = total_signal_energy(&sorted_nodes)?;
        let component_zero_energy = self
            .mode_energies
            .iter()
            .filter(|mode| mode.component_zero_mode)
            .map(|mode| mode.energy)
            .sum::<f64>();
        let nonzero_low_frequency_energy = self
            .mode_energies
            .iter()
            .filter(|mode| !mode.component_zero_mode)
            .map(|mode| mode.energy)
            .sum::<f64>();
        let summary = summarize_energy(
            total_signal_energy,
            component_zero_energy,
            nonzero_low_frequency_energy,
        )?;
        let projection_work = self.mode_energies.iter().try_fold(0_u64, |total, mode| {
            total
                .checked_add(mode.component_node_count as u64)
                .ok_or_else(|| GraphError::Invalid("projection work overflow".into()))
        })?;
        if self.format != "marklab.graph_sparse_radius_fourier_energy"
            || self.version != 1
            || self.graph_rule != "uniform_cell_exact_physical_radius"
            || self.laplacian_kind != "combinatorial_binary_sparse"
            || self.basis_method
                != "component_zero_modes_plus_shifted_laplacian_subspace_iteration_ritz"
            || self.component_policy
                != "retain_complete_component_zero_eigenspace_then_global_low_modes"
            || self.signal_policy
                != "project_exact_scalar_signal_with_component_mean_energy_separated"
            || self.claim_status != "experimental_descriptive_sparse_fourier_energy"
            || self.graph_digest.len() != 64
            || !self
                .graph_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.signal_digest != signal_digest(&sorted_nodes)
            || self.node_count != spec.basis.nodes.len()
            || self.component_sizes.iter().sum::<usize>() != self.node_count
            || self.radius_um.to_bits() != spec.basis.radius_um.to_bits()
            || self.requested_mode_count != spec.basis.mode_count
            || self.returned_mode_count != spec.basis.mode_count
            || self.basis_iterations != spec.basis.maximum_iterations
            || self.basis_residual_tolerance.to_bits() != spec.basis.residual_tolerance.to_bits()
            || !self.basis_maximum_residual_l2.is_finite()
            || self.basis_maximum_residual_l2 > spec.basis.residual_tolerance
            || !self.basis_orthogonality_max_abs_error.is_finite()
            || self.basis_orthogonality_max_abs_error > spec.basis.residual_tolerance.max(1e-10)
            || self.edge_count > spec.basis.maximum_edges
            || self.component_count > spec.basis.maximum_components
            || self.basis_candidate_pair_evaluations > spec.basis.maximum_candidate_pairs
            || self.basis_matrix_vector_work > spec.basis.maximum_matrix_vector_work
            || self.basis_orthogonalization_work > spec.basis.maximum_orthogonalization_work
            || self
                .mode_energies
                .iter()
                .filter(|mode| mode.component_zero_mode)
                .count()
                != self.component_count
            || self
                .mode_energies
                .iter()
                .take(self.component_count)
                .any(|mode| !mode.component_zero_mode)
            || self
                .mode_energies
                .iter()
                .skip(self.component_count)
                .any(|mode| mode.component_zero_mode)
            || self
                .mode_energies
                .windows(2)
                .any(|pair| pair[0].eigenvalue.total_cmp(&pair[1].eigenvalue).is_gt())
            || self.mode_energies.iter().enumerate().any(|(index, mode)| {
                mode.mode_index != index
                    || mode.component_index >= self.component_count
                    || mode.component_node_count != self.component_sizes[mode.component_index]
                    || !mode.eigenvalue.is_finite()
                    || mode.eigenvalue < 0.0
                    || !mode.coefficient.is_finite()
                    || !mode.energy.is_finite()
                    || mode.energy.to_bits() != mode.coefficient.powi(2).to_bits()
                    || (mode.component_zero_mode && mode.eigenvalue.to_bits() != 0.0_f64.to_bits())
            })
            || self.total_signal_energy.to_bits() != total_signal_energy.to_bits()
            || self.component_zero_energy.to_bits() != component_zero_energy.to_bits()
            || self.nonzero_low_frequency_energy.to_bits() != nonzero_low_frequency_energy.to_bits()
            || self.centered_signal_energy.to_bits() != summary.centered_signal_energy.to_bits()
            || self.captured_energy.to_bits() != summary.captured_energy.to_bits()
            || self.unresolved_centered_energy.to_bits()
                != summary.unresolved_centered_energy.to_bits()
            || option_bits(self.captured_total_energy_fraction)
                != option_bits(summary.captured_total_energy_fraction)
            || option_bits(self.captured_centered_energy_fraction)
                != option_bits(summary.captured_centered_energy_fraction)
            || option_bits(self.nonzero_low_frequency_fraction_of_total)
                != option_bits(summary.nonzero_low_frequency_fraction_of_total)
            || self.total_signal_status != summary.total_signal_status
            || self.centered_signal_status != summary.centered_signal_status
            || self.planned_projection_work != planned_projection_work
            || self.projection_work != projection_work
            || self.projection_work > spec.maximum_projection_work
            || self.estimated_fourier_retained_bytes != estimated_retained_bytes
            || self.estimated_fourier_retained_bytes > spec.maximum_fourier_retained_bytes
        {
            return Err(GraphError::Invalid(
                "sparse Fourier result does not match its typed specification".into(),
            ));
        }
        Ok(())
    }
}

pub fn graph_sparse_radius_fourier_energy_workflow(
    spec: GraphSparseRadiusFourierEnergySpec,
) -> Result<GraphSparseRadiusFourierEnergyResult, GraphError> {
    let planned_projection_work = planned_projection_work(&spec)?;
    if planned_projection_work > spec.maximum_projection_work {
        return Err(GraphError::Invalid(
            "projection work exceeds caller maximum".into(),
        ));
    }
    let estimated_fourier_retained_bytes = estimated_retained_bytes(spec.basis.mode_count)?;
    if estimated_fourier_retained_bytes > spec.maximum_fourier_retained_bytes {
        return Err(GraphError::Invalid(
            "Fourier retained bytes exceed caller maximum".into(),
        ));
    }

    let mut sorted_nodes = spec.basis.nodes.clone();
    sorted_nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let total_signal_energy = total_signal_energy(&sorted_nodes)?;
    let basis = graph_sparse_radius_basis_workflow(spec.basis.clone())?;
    let (mode_energies, projection_work) = project_modes(&basis, &sorted_nodes)?;
    let component_zero_energy = mode_energies
        .iter()
        .filter(|mode| mode.component_zero_mode)
        .map(|mode| mode.energy)
        .sum::<f64>();
    let nonzero_low_frequency_energy = mode_energies
        .iter()
        .filter(|mode| !mode.component_zero_mode)
        .map(|mode| mode.energy)
        .sum::<f64>();
    let summary = summarize_energy(
        total_signal_energy,
        component_zero_energy,
        nonzero_low_frequency_energy,
    )?;

    Ok(GraphSparseRadiusFourierEnergyResult {
        format: "marklab.graph_sparse_radius_fourier_energy".into(),
        version: 1,
        graph_digest: basis.graph_digest,
        signal_digest: signal_digest(&sorted_nodes),
        graph_rule: basis.graph_rule,
        laplacian_kind: basis.laplacian_kind,
        basis_method: basis.basis_method,
        component_policy: basis.component_policy,
        signal_policy: "project_exact_scalar_signal_with_component_mean_energy_separated".into(),
        node_count: basis.node_count,
        edge_count: basis.edge_count,
        isolated_node_count: basis.isolated_node_count,
        component_count: basis.component_count,
        component_sizes: basis.component_sizes,
        radius_um: basis.radius_um,
        requested_mode_count: basis.requested_mode_count,
        returned_mode_count: basis.returned_mode_count,
        basis_iterations: basis.iterations,
        basis_residual_tolerance: basis.residual_tolerance,
        basis_maximum_residual_l2: basis.maximum_residual_l2,
        basis_orthogonality_max_abs_error: basis.orthogonality_max_abs_error,
        mode_energies,
        total_signal_energy,
        component_zero_energy,
        centered_signal_energy: summary.centered_signal_energy,
        nonzero_low_frequency_energy,
        captured_energy: summary.captured_energy,
        unresolved_centered_energy: summary.unresolved_centered_energy,
        captured_total_energy_fraction: summary.captured_total_energy_fraction,
        captured_centered_energy_fraction: summary.captured_centered_energy_fraction,
        nonzero_low_frequency_fraction_of_total: summary.nonzero_low_frequency_fraction_of_total,
        total_signal_status: summary.total_signal_status.into(),
        centered_signal_status: summary.centered_signal_status.into(),
        planned_projection_work,
        projection_work,
        estimated_fourier_retained_bytes,
        basis_candidate_pair_evaluations: basis.candidate_pair_evaluations,
        basis_matrix_vector_work: basis.matrix_vector_work,
        basis_orthogonalization_work: basis.orthogonalization_work,
        claim_status: "experimental_descriptive_sparse_fourier_energy".into(),
    })
}

fn planned_projection_work(spec: &GraphSparseRadiusFourierEnergySpec) -> Result<u64, GraphError> {
    if spec.maximum_projection_work == 0 || spec.maximum_fourier_retained_bytes == 0 {
        return Err(GraphError::Invalid(
            "sparse Fourier resource limits must be positive".into(),
        ));
    }
    (spec.basis.nodes.len() as u64)
        .checked_mul(spec.basis.mode_count as u64)
        .ok_or_else(|| GraphError::Invalid("planned projection work overflow".into()))
}

fn estimated_retained_bytes(mode_count: usize) -> Result<u64, GraphError> {
    (mode_count as u64)
        .checked_mul(SUMMARY_MODE_BYTES)
        .and_then(|bytes| bytes.checked_add(SUMMARY_BASE_BYTES))
        .ok_or_else(|| GraphError::Invalid("Fourier retained byte estimate overflow".into()))
}

fn project_modes(
    basis: &GraphSparseRadiusBasisResult,
    sorted_nodes: &[GraphNodeInput],
) -> Result<(Vec<SparseRadiusFourierModeEnergy>, u64), GraphError> {
    let mut projection_work = 0_u64;
    let mut energies = Vec::with_capacity(basis.modes.len());
    for mode in &basis.modes {
        let component = &basis.components[mode.component_index];
        let coefficient = mode
            .values_by_component_node
            .iter()
            .zip(&component.node_indices)
            .map(|(basis_value, node_index)| basis_value * sorted_nodes[*node_index].signal)
            .sum::<f64>();
        let energy = coefficient.powi(2);
        if !coefficient.is_finite() || !energy.is_finite() {
            return Err(GraphError::Numerical(
                "sparse Fourier coefficient or energy is non-finite".into(),
            ));
        }
        projection_work = projection_work
            .checked_add(component.node_indices.len() as u64)
            .ok_or_else(|| GraphError::Invalid("projection work overflow".into()))?;
        energies.push(SparseRadiusFourierModeEnergy {
            mode_index: mode.mode_index,
            component_index: mode.component_index,
            component_node_count: component.node_indices.len(),
            eigenvalue: mode.eigenvalue,
            coefficient,
            energy,
            component_zero_mode: mode.component_zero_mode,
        });
    }
    Ok((energies, projection_work))
}

fn total_signal_energy(nodes: &[GraphNodeInput]) -> Result<f64, GraphError> {
    let energy = nodes.iter().map(|node| node.signal.powi(2)).sum::<f64>();
    if !energy.is_finite() || (energy == 0.0 && nodes.iter().any(|node| node.signal != 0.0)) {
        return Err(GraphError::Numerical(
            "scalar signal energy is non-finite or underflowed".into(),
        ));
    }
    Ok(energy)
}

fn summarize_energy(
    total: f64,
    component_zero: f64,
    nonzero_low: f64,
) -> Result<EnergySummary, GraphError> {
    if [total, component_zero, nonzero_low]
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(GraphError::Numerical(
            "sparse Fourier energy summary is non-finite or negative".into(),
        ));
    }
    let tolerance = 128.0 * f64::EPSILON * total.max(1.0);
    let centered_raw = total - component_zero;
    if centered_raw < -tolerance {
        return Err(GraphError::Numerical(
            "component-zero energy exceeds total signal energy".into(),
        ));
    }
    let centered = if centered_raw.abs() <= tolerance {
        0.0
    } else {
        centered_raw
    };
    let unresolved_raw = centered - nonzero_low;
    if unresolved_raw < -tolerance {
        return Err(GraphError::Numerical(
            "captured nonzero energy exceeds centered signal energy".into(),
        ));
    }
    let unresolved = if unresolved_raw.abs() <= tolerance {
        0.0
    } else {
        unresolved_raw
    };
    let captured_raw = component_zero + nonzero_low;
    if !captured_raw.is_finite() {
        return Err(GraphError::Numerical(
            "captured sparse Fourier energy is non-finite".into(),
        ));
    }
    let captured = if captured_raw > total && captured_raw - total <= tolerance {
        total
    } else {
        captured_raw
    };
    Ok(EnergySummary {
        centered_signal_energy: centered,
        captured_energy: captured,
        unresolved_centered_energy: unresolved,
        captured_total_energy_fraction: (total > 0.0).then_some(captured / total),
        captured_centered_energy_fraction: (centered > 0.0).then_some(nonzero_low / centered),
        nonzero_low_frequency_fraction_of_total: (total > 0.0).then_some(nonzero_low / total),
        total_signal_status: if total == 0.0 { "zero" } else { "positive" },
        centered_signal_status: if centered == 0.0 {
            "zero_within_roundoff"
        } else {
            "positive"
        },
    })
}

fn signal_digest(nodes: &[GraphNodeInput]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"marklab-sparse-radius-scalar-signal-v1\0");
    for node in nodes {
        digest.update((node.id.len() as u64).to_be_bytes());
        digest.update(node.id.as_bytes());
        digest.update(node.signal.to_bits().to_be_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn option_bits(value: Option<f64>) -> Option<u64> {
    value.map(f64::to_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_zero_and_component_constant_signals_have_explicit_fraction_states() {
        let zero = summarize_energy(0.0, 0.0, 0.0).expect("zero summary");
        assert_eq!(zero.total_signal_status, "zero");
        assert_eq!(zero.centered_signal_status, "zero_within_roundoff");
        assert_eq!(zero.captured_total_energy_fraction, None);
        assert_eq!(zero.captured_centered_energy_fraction, None);

        let constant = summarize_energy(4.0, 4.0, 0.0).expect("constant summary");
        assert_eq!(constant.total_signal_status, "positive");
        assert_eq!(constant.centered_signal_status, "zero_within_roundoff");
        assert_eq!(constant.captured_total_energy_fraction, Some(1.0));
        assert_eq!(constant.captured_centered_energy_fraction, None);
    }
}

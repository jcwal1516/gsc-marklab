use super::{
    admission::{component_graphs, plan_work, validate_spec},
    model::{
        GraphSparseRadiusBasisResult, GraphSparseRadiusBasisSpec, SparseRadiusBasisComponent,
        SparseRadiusBasisMode,
    },
    numerical::{approximate_component_modes, orient_mode, orthogonality_error, CandidateMode},
};
use crate::{
    sparse_radius_graph::{build_sparse_radius_graph, graph_digest},
    GraphError,
};

const PRIMARY_SEED_STREAM: u64 = 0x12da_f125_2b76_4f0b;
const COMPLEMENT_SEED_STREAM: u64 = 0xa541_69c8_7d9e_31f3;

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
    let inspection_modes = nonzero_needed.saturating_add(1);
    let cluster_tolerance =
        eigenvalue_cluster_tolerance(spec.residual_tolerance, graph.nodes.len());
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
        let available_nonzero = component.global_nodes.len().saturating_sub(1);
        let primary_count = nonzero_needed.min(available_nonzero);
        let component_stream = (component_index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        // A dense block avoids the structural blind spot of coordinate-prefix seeds. Reserve the
        // final admitted candidate for a separately seeded search in the block's complement. A
        // clearly lower complement mode invalidates the primary enumeration; it is not silently
        // substituted into a result whose remaining omitted spectrum has not been inspected.
        let mut component_modes = if primary_count == 0 {
            Vec::new()
        } else {
            let remaining_rotations = spec
                .maximum_ritz_rotations
                .checked_sub(rotations)
                .ok_or_else(|| GraphError::Invalid("Ritz rotation count overflow".into()))?;
            let (modes, used_rotations) = approximate_component_modes(
                component_index,
                component,
                primary_count,
                spec.maximum_iterations,
                remaining_rotations,
                PRIMARY_SEED_STREAM ^ component_stream,
                &[],
            )?;
            rotations = rotations
                .checked_add(used_rotations)
                .ok_or_else(|| GraphError::Invalid("Ritz rotation count overflow".into()))?;
            modes
        };
        if available_nonzero > primary_count {
            let remaining_rotations = spec
                .maximum_ritz_rotations
                .checked_sub(rotations)
                .ok_or_else(|| GraphError::Invalid("Ritz rotation count overflow".into()))?;
            let (mut complement, used_rotations) = approximate_component_modes(
                component_index,
                component,
                1,
                spec.maximum_iterations,
                remaining_rotations,
                COMPLEMENT_SEED_STREAM ^ component_stream,
                &component_modes,
            )?;
            let primary_maximum = component_modes
                .iter()
                .map(|mode| mode.eigenvalue)
                .fold(f64::NEG_INFINITY, f64::max);
            let complement_mode = complement.first().ok_or_else(|| {
                GraphError::Numerical("sparse basis complement probe did not produce a mode".into())
            })?;
            if component_modes
                .iter()
                .all(|mode| mode.residual_l2 <= spec.residual_tolerance)
                && complement_mode.residual_l2 <= spec.residual_tolerance
                && primary_maximum - complement_mode.eigenvalue > cluster_tolerance
            {
                return Err(GraphError::Numerical(
                    "sparse basis complement probe found a missed lower mode in the primary block"
                        .into(),
                ));
            }
            rotations = rotations
                .checked_add(used_rotations)
                .ok_or_else(|| GraphError::Invalid("Ritz rotation count overflow".into()))?;
            component_modes.append(&mut complement);
        }
        debug_assert_eq!(
            component_modes.len(),
            inspection_modes.min(available_nonzero)
        );
        candidates.append(&mut component_modes);
    }
    candidates.sort_by(|left, right| {
        left.eigenvalue
            .total_cmp(&right.eigenvalue)
            .then(right.zero_mode.cmp(&left.zero_mode))
            .then(left.component_index.cmp(&right.component_index))
    });
    if candidates
        .iter()
        .any(|candidate| !candidate.zero_mode && candidate.residual_l2 > spec.residual_tolerance)
    {
        return Err(GraphError::Numerical(
            "sparse basis cutoff boundary is unavailable because a retained or inspection mode exceeds the residual tolerance"
                .into(),
        ));
    }
    let cutoff_boundary_gap = if spec.mode_count == graph.nodes.len() {
        None
    } else {
        let retained = candidates.get(spec.mode_count - 1).ok_or_else(|| {
            GraphError::Numerical("sparse basis did not produce the requested modes".into())
        })?;
        let omitted = candidates.get(spec.mode_count).ok_or_else(|| {
            GraphError::Numerical(
                "sparse basis cutoff boundary is unavailable within the bounded candidate work"
                    .into(),
            )
        })?;
        let gap = omitted.eigenvalue - retained.eigenvalue;
        if !gap.is_finite() || gap <= cluster_tolerance {
            return Err(GraphError::Numerical(
                "sparse basis cutoff intersects an unresolved repeated or near-repeated eigenspace"
                    .into(),
            ));
        }
        Some(gap)
    };
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
        version: 2,
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
        eigenvalue_cluster_policy:
            "adjacent_ritz_intervals_overlap_under_two_residuals_plus_roundoff".into(),
        eigenvalue_cluster_tolerance: cluster_tolerance,
        cutoff_boundary_gap,
        cutoff_status: if spec.mode_count == graph.nodes.len() {
            "complete_spectrum"
        } else {
            "resolved_by_gap"
        }
        .into(),
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

pub(super) fn eigenvalue_cluster_tolerance(residual_tolerance: f64, node_count: usize) -> f64 {
    // Each converged symmetric Ritz pair has an eigenvalue uncertainty no larger than its
    // residual norm. Two such intervals can therefore overlap by two requested residuals.
    // The additional term covers f64 comparisons at the combinatorial-Laplacian scale, whose
    // spectral norm is bounded by twice the maximum degree and hence by 2 * node_count.
    2.0 * residual_tolerance + 256.0 * f64::EPSILON * (node_count.max(1) as f64)
}

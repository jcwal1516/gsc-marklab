use super::{graph_sparse_radius_basis_workflow, GraphSparseRadiusBasisSpec};
use crate::GraphNodeInput;

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

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn sparse_radius_basis_matches_the_path_laplacian_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("basis.json");
    let output = directory.path().join("result.json");
    let spec = serde_json::json!({
        "nodes": (0..5).map(|index| serde_json::json!({
            "id": format!("cell-{index}"),
            "coordinates_um": [index as f64, 0.0],
            "signal": if index == 2 { 1.0 } else { 0.0 }
        })).collect::<Vec<_>>(),
        "radius_um": 1.1,
        "mode_count": 3,
        "maximum_iterations": 96,
        "residual_tolerance": 1e-9,
        "maximum_nodes": 5,
        "maximum_candidate_pairs": 10,
        "maximum_edges": 4,
        "maximum_components": 1,
        "maximum_matrix_vector_work": 10_000,
        "maximum_orthogonalization_work": 100_000,
        "maximum_ritz_rotations": 1_000,
        "maximum_working_bytes": 64_000,
        "maximum_retained_bytes": 64_000
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-basis",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("result JSON");
    assert_eq!(result["format"], "marklab.graph_sparse_radius_basis");
    assert_eq!(result["graph_rule"], "uniform_cell_exact_physical_radius");
    assert_eq!(result["laplacian_kind"], "combinatorial_binary_sparse");
    assert_eq!(result["node_count"], 5);
    assert_eq!(result["edge_count"], 4);
    assert_eq!(result["component_count"], 1);
    assert_eq!(result["returned_mode_count"], 3);
    assert!(result["maximum_residual_l2"].as_f64().unwrap() <= 1e-9);
    assert!(result["orthogonality_max_abs_error"].as_f64().unwrap() <= 1e-10);

    let expected = [
        0.0,
        2.0 - 2.0 * (std::f64::consts::PI / 5.0).cos(),
        2.0 - 2.0 * (2.0 * std::f64::consts::PI / 5.0).cos(),
    ];
    let modes = result["modes"].as_array().unwrap();
    for (mode, expected_eigenvalue) in modes.iter().zip(expected) {
        let eigenvalue = mode["eigenvalue"].as_f64().unwrap();
        assert!((eigenvalue - expected_eigenvalue).abs() <= 1e-9);
        let values = mode["values_by_component_node"].as_array().unwrap();
        assert_eq!(values.len(), 5);
        assert!(values
            .iter()
            .all(|value| value.as_f64().unwrap().is_finite()));
    }
    assert_eq!(modes[0]["component_zero_mode"], true);
    assert_eq!(modes[1]["component_zero_mode"], false);
    assert_eq!(modes[2]["component_zero_mode"], false);
}

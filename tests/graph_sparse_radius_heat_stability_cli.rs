#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use marklab_graph::GraphNodeInput;

fn nodes() -> Vec<GraphNodeInput> {
    [1.0, -0.5, 2.0, 0.25, -1.0]
        .into_iter()
        .enumerate()
        .map(|(index, signal)| GraphNodeInput {
            id: format!("cell-{index}"),
            coordinates_um: [index as f64, 0.0],
            signal,
        })
        .collect()
}

#[test]
fn sparse_heat_stability_has_an_exact_zero_jitter_oracle_and_aggregate_bounds() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("stability.json");
    let output = directory.path().join("result.json");
    let mut spec = serde_json::json!({
        "nodes": nodes(),
        "radius_um": 1.1,
        "time": 0.35,
        "tolerance": 1e-8,
        "maximum_order": 64,
        "maximum_nodes": 5,
        "maximum_candidate_pairs": 10,
        "maximum_edges": 4,
        "maximum_matrix_vector_work": 10_000,
        "maximum_working_bytes": 16_384,
        "perturbation_replicates": 4,
        "maximum_coordinate_jitter_um": 0.0,
        "seed": 20260829,
        "maximum_total_candidate_pairs": 50,
        "maximum_total_matrix_vector_work": 50_000,
        "maximum_relative_l2_change": 0.0
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-heat-stability",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.graph_sparse_radius_heat_stability"
    );
    assert_eq!(result["baseline"]["edge_count"], 4);
    assert_eq!(result["perturbations"].as_array().unwrap().len(), 4);
    assert_eq!(result["maximum_relative_l2_change"], 0.0);
    assert_eq!(result["stable_under_declared_threshold"], true);
    assert!(result["perturbations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| {
            row["relative_filtered_l2_change"] == 0.0
                && row["graph_digest"] == result["baseline"]["graph_digest"]
        }));

    spec["maximum_total_candidate_pairs"] = serde_json::json!(49);
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).unwrap();
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "sparse-radius-heat-stability",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "aggregate candidate work exceeds caller maximum",
        ));
}

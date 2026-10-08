#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn graph_dirichlet_energy_matches_weighted_vector_hand_oracle() {
    let directory = tempfile::tempdir().unwrap();
    let nodes = directory.path().join("nodes.csv");
    let edges = directory.path().join("edges.csv");
    let output = directory.path().join("output.json");
    fs::write(&nodes, "node_id,signal_0,signal_1\na,0,0\nb,1,0\nc,3,4\n").unwrap();
    fs::write(&edges, "left_node_id,right_node_id,weight\na,b,1\nb,c,2\n").unwrap();

    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "graph-dirichlet-energy",
            "--nodes",
            nodes.to_str().unwrap(),
            "--edges",
            edges.to_str().unwrap(),
            "--laplacian",
            "combinatorial",
            "--normalization",
            "signal",
            "--maximum-component-edge-visits",
            "6",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.graph_dirichlet_energy");
    assert_eq!(result["version"], 1);
    assert_eq!(result["laplacian"], "combinatorial");
    assert_eq!(result["normalization"], "signal");
    assert_eq!(result["node_count"], 3);
    assert_eq!(result["edge_count"], 2);
    assert_eq!(result["signal_dimension"], 2);
    assert_eq!(result["numerator"], 41.0);
    assert!((result["denominator"].as_f64().unwrap() - 46.0 / 3.0).abs() < 1e-12);
    assert!((result["energy"].as_f64().unwrap() - 123.0 / 46.0).abs() < 1e-12);
    assert_eq!(result["graph_digest"].as_str().unwrap().len(), 64);
}

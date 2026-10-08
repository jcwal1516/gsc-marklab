#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn normalized_hypergraph_laplacian_matches_two_edge_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("hypergraph.json");
    fs::write(
        &input,
        r#"{
  "nodes":[
    {"id":"a","signal":1.0},
    {"id":"b","signal":0.0},
    {"id":"c","signal":-1.0}
  ],
  "hyperedges":[
    {"id":"left","hyperedge_type":"niche","weight":1.0,"members":[{"node_id":"a","membership":1.0},{"node_id":"b","membership":1.0}]},
    {"id":"right","hyperedge_type":"niche","weight":1.0,"members":[{"node_id":"b","membership":1.0},{"node_id":"c","membership":1.0}]}
  ],
  "epsilon":0.000000000001,
  "maximum_incidence_entries":100
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "hypergraph",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.hypergraph_signal");
    assert_eq!(result["node_degrees"], serde_json::json!([1.0, 2.0, 1.0]));
    assert_eq!(result["hyperedge_degrees"], serde_json::json!([2.0, 2.0]));
    assert!((result["laplacian"][0][0].as_f64().unwrap() - 0.5).abs() < 1e-12);
    assert!((result["laplacian"][0][1].as_f64().unwrap() + 1.0 / (8.0_f64).sqrt()).abs() < 1e-12);
    assert!((result["signal_smoothness"].as_f64().unwrap() - 0.5).abs() < 1e-12);
    assert_eq!(result["incidence_entries"], 4);
}

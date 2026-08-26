#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn local_embedding_roughness_matches_weighted_hand_oracle_and_retains_island() {
    let directory = tempfile::tempdir().unwrap();
    let nodes = directory.path().join("nodes.csv");
    let edges = directory.path().join("edges.csv");
    let output = directory.path().join("output.json");
    fs::write(
        &nodes,
        "node_id,signal_0,signal_1\na,0,0\nb,1,0\nc,3,4\nd,9,9\n",
    )
    .unwrap();
    fs::write(&edges, "left_node_id,right_node_id,weight\na,b,1\nb,c,2\n").unwrap();

    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "local-embedding-roughness",
            "--nodes",
            nodes.to_str().unwrap(),
            "--edges",
            edges.to_str().unwrap(),
            "--epsilon",
            "0.000000000001",
            "--maximum-component-edge-visits",
            "6",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.local_embedding_roughness");
    assert_eq!(result["version"], 1);
    assert_eq!(
        result["claim_status"],
        "experimental_descriptive_no_hotspot_inference"
    );
    assert_eq!(result["rows"][0]["node_id"], "a");
    assert_eq!(result["rows"][0]["local_roughness"], 1.0);
    assert!((result["rows"][1]["local_roughness"].as_f64().unwrap() - 41.0 / 3.0).abs() < 1e-12);
    assert_eq!(result["rows"][2]["local_roughness"], 20.0);
    assert_eq!(result["rows"][3]["node_id"], "d");
    assert_eq!(result["rows"][3]["neighbor_count"], 0);
    assert_eq!(result["rows"][3]["weighted_degree"], 0.0);
    assert_eq!(result["rows"][3]["local_roughness"], 0.0);
    assert_eq!(result["rows"][3]["status"], "isolated_node");
}

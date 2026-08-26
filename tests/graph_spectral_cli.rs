#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn path_graph_fourier_matches_the_exact_three_node_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("graph.json");
    fs::write(
        &input,
        r#"{
  "nodes": [
    {"id":"c","coordinates_um":[2.0,0.0],"signal":-1.0},
    {"id":"a","coordinates_um":[0.0,0.0],"signal":1.0},
    {"id":"b","coordinates_um":[1.0,0.0],"signal":0.0}
  ],
  "radius_um": 1.1,
  "weight": "binary",
  "laplacian": "combinatorial",
  "bands": [
    {"id":"low","minimum":0.0,"maximum":0.5},
    {"id":"middle","minimum":0.5,"maximum":1.5},
    {"id":"high","minimum":1.5,"maximum":3.5}
  ],
  "maximum_pairs": 100
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "spectral",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.graph_spectral");
    assert_eq!(result["nodes"][0]["id"], "a");
    assert_eq!(result["edges"].as_array().unwrap().len(), 2);
    let eigenvalues = result["spectrum"]["eigenvalues"].as_array().unwrap();
    for (actual, expected) in eigenvalues.iter().zip([0.0, 1.0, 3.0]) {
        assert!((actual.as_f64().unwrap() - expected).abs() < 1e-10);
    }
    let bands = result["frequency_bands"].as_array().unwrap();
    assert!(bands[0]["energy"].as_f64().unwrap().abs() < 1e-10);
    assert!((bands[1]["energy"].as_f64().unwrap() - 2.0).abs() < 1e-10);
    assert!(bands[2]["energy"].as_f64().unwrap().abs() < 1e-10);
    assert_eq!(result["work"]["pair_evaluations"], 3);
}

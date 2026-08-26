#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn constant_signal_matches_the_complete_tie_erl_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("null.json");
    fs::write(
        &input,
        r#"{
  "graph": {
    "nodes": [
      {"id":"a","coordinates_um":[0.0,0.0],"signal":1.0},
      {"id":"b","coordinates_um":[1.0,0.0],"signal":1.0},
      {"id":"c","coordinates_um":[2.0,0.0],"signal":1.0},
      {"id":"d","coordinates_um":[3.0,0.0],"signal":1.0}
    ],
    "radius_um":1.1,
    "weight":"binary",
    "laplacian":"combinatorial",
    "bands":[
      {"id":"low","minimum":0.0,"maximum":0.5},
      {"id":"high","minimum":0.5,"maximum":4.1}
    ],
    "maximum_pairs":100
  },
  "node_strata":[
    {"node_id":"a","stratum":"all"},
    {"node_id":"b","stratum":"all"},
    {"node_id":"c","stratum":"all"},
    {"node_id":"d","stratum":"all"}
  ],
  "permutations":39,
  "alpha":0.05,
  "seed":123
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "spectrum-null",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.graph_spectrum_null");
    assert_eq!(result["p_global"], 1.0);
    assert_eq!(result["low_frequency_p_value"], 1.0);
    assert!((result["bands"][0]["observed"].as_f64().unwrap() - 4.0).abs() < 1e-10);
    assert!(result["bands"][1]["observed"].as_f64().unwrap().abs() < 1e-10);
    assert_eq!(result["bands"][0]["lower"], result["bands"][0]["observed"]);
    assert_eq!(result["bands"][0]["upper"], result["bands"][0]["observed"]);
    assert_eq!(result["permutations_completed"], 39);
    assert_eq!(
        result["permutation_unit"],
        "complete_signal_rows_within_declared_strata"
    );
}

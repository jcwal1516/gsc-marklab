#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn typed_triangle_count_adjacency_and_complete_tie_null_match_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("motif.json");
    fs::write(
        &input,
        r#"{
  "nodes":[
    {"id":"a","label":"cell","stratum":"all"},
    {"id":"b","label":"cell","stratum":"all"},
    {"id":"c","label":"vessel","stratum":"all"}
  ],
  "edges":[
    {"source_id":"a","target_id":"b"},
    {"source_id":"a","target_id":"c"},
    {"source_id":"b","target_id":"c"}
  ],
  "motif_id":"cell-cell-vessel-triangle",
  "required_labels":["cell","cell","vessel"],
  "permutations":39,
  "seed":77,
  "maximum_triples":100
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "motif-triangle",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.typed_triangle_motif");
    assert_eq!(result["observed_count"], 1);
    assert_eq!(result["instances"], serde_json::json!([["a", "b", "c"]]));
    assert_eq!(result["motif_adjacency"][0], serde_json::json!([0, 1, 1]));
    assert_eq!(result["motif_adjacency"][1], serde_json::json!([1, 0, 1]));
    assert_eq!(result["p_value_upper"], 1.0);
    assert_eq!(result["null_counts"].as_array().unwrap().len(), 39);
    assert!(result["null_counts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|value| value == 1));
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn farthest_point_witness_tree_matches_line_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("witness.json");
    fs::write(
        &input,
        r#"{
  "points":[
    {"id":"a","coordinates_um":[0.0]},
    {"id":"b","coordinates_um":[1.0]},
    {"id":"c","coordinates_um":[2.0]},
    {"id":"d","coordinates_um":[3.0]},
    {"id":"e","coordinates_um":[4.0]}
  ],
  "landmark_method":"farthest_point",
  "landmark_count":3,
  "maximum_dimension":1,
  "nu":0,
  "max_scale_um":1.0,
  "coefficient_field":2,
  "maximum_simplices":100,
  "timeout_seconds":30
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "witness-persistence",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.witness_persistence");
    assert_eq!(result["backend"]["version"], "3.13.0");
    assert_eq!(result["landmark_ids"], serde_json::json!(["a", "e", "c"]));
    assert!(
        (result["approximation"]["coverage_radius_um"]
            .as_f64()
            .unwrap()
            - 1.0)
            .abs()
            < 1e-12
    );
    assert_eq!(result["approximation"]["nu"], 0);
    assert_eq!(
        result["filtration"]["simplex_counts_by_dimension"],
        serde_json::json!([3, 2])
    );
    assert_eq!(result["filtration"]["validation_status"], "passed");
    assert_eq!(
        result["persistence"]["by_dimension"][0]["essential_count"],
        1
    );
    assert_eq!(
        result["persistence"]["by_dimension"][0]["finite_pairs"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(result["claim_status"], "experimental_witness_approximation");
}

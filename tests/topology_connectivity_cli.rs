#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn three_point_line_connectivity_transition_matches_union_find_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("connectivity.json");
    fs::write(
        &input,
        r#"{
  "points":[
    {"id":"a","coordinates_um":[0.0,0.0]},
    {"id":"b","coordinates_um":[1.0,0.0]},
    {"id":"c","coordinates_um":[2.0,0.0]}
  ],
  "radii_um":[0.5,1.0,2.0],
  "window":{"minimum_um":[0.0,-0.5],"maximum_um":[2.0,0.5]},
  "boundary_tolerance_um":0.000000000001,
  "maximum_pairs":100
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "connectivity",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.connectivity_transition");
    assert_eq!(result["curves"][0]["component_count"], 3);
    assert!((result["curves"][0]["largest_fraction"].as_f64().unwrap() - 1.0 / 3.0).abs() < 1e-12);
    assert!((result["curves"][0]["susceptibility"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-12);
    assert_eq!(result["curves"][0]["spans_left_right"], false);
    assert_eq!(result["curves"][1]["component_count"], 1);
    assert_eq!(result["curves"][1]["largest_fraction"], 1.0);
    assert_eq!(result["curves"][1]["spans_left_right"], true);
    assert_eq!(result["critical_radius_um"], 1.0);
    assert_eq!(
        result["critical_radius_rule"],
        "first_declared_boundary_spanning_radius"
    );
    assert_eq!(
        result["claim_status"],
        "experimental_finite_size_connectivity"
    );
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn scattering_and_declared_stability_match_path_signal_oracles() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("scattering.json");
    fs::write(
        &input,
        r#"{
  "graph": {
    "nodes": [
      {"id":"a","coordinates_um":[0.0,0.0],"signal":1.0},
      {"id":"b","coordinates_um":[1.0,0.0],"signal":0.0},
      {"id":"c","coordinates_um":[2.0,0.0],"signal":-1.0}
    ],
    "radius_um":1.1,
    "weight":"binary",
    "laplacian":"combinatorial",
    "bands":[{"id":"all","minimum":0.0,"maximum":3.5}],
    "maximum_pairs":100
  },
  "scales":[1.0,2.0],
  "maximum_order":1,
  "stability_ratio_tolerance":1.0,
  "perturbations":[
    {"id":"constant-shift","signal_delta":[0.1,0.1,0.1]}
  ]
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "scattering",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.graph_scattering");
    let features = result["features"].as_array().unwrap();
    assert_eq!(features[0]["path"], serde_json::json!([]));
    assert!(features[0]["value"].as_f64().unwrap().abs() < 1e-10);
    assert_eq!(features[1]["path"], serde_json::json!([1.0]));
    let expected = 2.0 * (-1.0_f64).exp() / 3.0;
    assert!((features[1]["value"].as_f64().unwrap() - expected).abs() < 1e-10);
    let stability = &result["stability"][0];
    assert!((stability["feature_delta"].as_f64().unwrap() - 0.1).abs() < 1e-10);
    assert!(
        (stability["perturbation_magnitude"].as_f64().unwrap() - 0.03_f64.sqrt()).abs() < 1e-10
    );
    assert!(stability["passed"].as_bool().unwrap());
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn exact_heat_kernel_has_identity_zero_time_and_diffusion_oracles() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("heat.json");
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
  "times":[0.0,1.0],
  "diffusion_pairs":[{"source_id":"a","target_id":"c"}]
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "heat",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.graph_heat");
    let at_zero = &result["times"][0];
    assert_eq!(at_zero["time"], 0.0);
    for row in 0..3 {
        for column in 0..3 {
            let expected = if row == column { 1.0 } else { 0.0 };
            assert!((at_zero["kernel"][row][column].as_f64().unwrap() - expected).abs() < 1e-10);
        }
    }
    for (actual, expected) in at_zero["applied_signal"]
        .as_array()
        .unwrap()
        .iter()
        .zip([1.0, 0.0, -1.0])
    {
        assert!((actual.as_f64().unwrap() - expected).abs() < 1e-10);
    }
    for value in at_zero["heat_kernel_signature"].as_array().unwrap() {
        assert!((value.as_f64().unwrap() - 1.0).abs() < 1e-10);
    }
    assert!(
        (at_zero["diffusion_distances"][0]["distance"]
            .as_f64()
            .unwrap()
            - 2.0_f64.sqrt())
        .abs()
            < 1e-10
    );
    let at_one = &result["times"][1];
    for row in at_one["kernel"].as_array().unwrap() {
        let sum = row
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .sum::<f64>();
        assert!((sum - 1.0).abs() < 1e-10);
    }
    let smoothed_energy = at_one["applied_signal"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap().powi(2))
        .sum::<f64>();
    assert!(smoothed_energy < 2.0);
}

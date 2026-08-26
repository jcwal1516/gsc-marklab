#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn path_signal_wavelet_energy_matches_its_single_eigenmode_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("wavelet.json");
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
  "lowpass_scale":1.0
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "wavelet",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.graph_spectral_wavelet");
    assert_eq!(result["kernel"], "x_exp_minus_x");
    let expected_scale_one_energy = 2.0 * (-2.0_f64).exp();
    assert!(
        (result["scales"][0]["energy"].as_f64().unwrap() - expected_scale_one_energy).abs() < 1e-10
    );
    assert!((result["scaling_energy"].as_f64().unwrap() - expected_scale_one_energy).abs() < 1e-10);
    assert!(result["scales"][1]["energy"].as_f64().unwrap() < expected_scale_one_energy);
    assert_eq!(result["nodes"][0]["id"], "a");
}

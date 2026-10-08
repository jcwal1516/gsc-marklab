#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn adaptive_chebyshev_heat_matches_the_exact_path_eigenmode() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("approx.json");
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
  "time":1.0,
  "tolerance":0.00000001,
  "maximum_order":32
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "chebyshev-heat",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.graph_chebyshev_heat");
    assert!(result["selected_order"].as_u64().unwrap() <= 32);
    assert!(result["verified_grid_error"].as_f64().unwrap() <= 1e-8);
    assert_eq!(
        result["error_bound_kind"],
        "dense_grid_plus_truncated_reference_tail"
    );
    assert!(result["maximum_signal_error"].as_f64().unwrap() <= 1e-8);
    let expected = (-1.0_f64).exp();
    let applied = result["approximate_signal"].as_array().unwrap();
    assert!((applied[0].as_f64().unwrap() - expected).abs() <= 1e-8);
    assert!(applied[1].as_f64().unwrap().abs() <= 1e-8);
    assert!((applied[2].as_f64().unwrap() + expected).abs() <= 1e-8);
    assert_eq!(result["algorithm"], "adaptive_chebyshev_heat");
}

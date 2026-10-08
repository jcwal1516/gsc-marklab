#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn diffusion_wavelet_separates_the_path_lambda_one_signal_at_level_two() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("diffusion.json");
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
  "tolerance":0.5,
  "maximum_levels":4
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "diffusion-wavelet",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.graph_diffusion_wavelet");
    let levels = result["tree"]["levels"].as_array().unwrap();
    assert_eq!(levels.len(), 2);
    assert_eq!(levels[0]["retained_rank"], 2);
    assert_eq!(levels[1]["retained_rank"], 1);
    assert_eq!(
        result["transform"]["detail_by_level"][0]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let second_detail = result["transform"]["detail_by_level"][1]
        .as_array()
        .unwrap();
    assert_eq!(second_detail.len(), 1);
    assert!((second_detail[0].as_f64().unwrap().abs() - 2.0_f64.sqrt()).abs() < 1e-10);
    assert!(result["transform"]["coarse"][0].as_f64().unwrap().abs() < 1e-10);
    assert!(
        result["transform"]["reconstruction_max_abs_error"]
            .as_f64()
            .unwrap()
            < 1e-10
    );
}

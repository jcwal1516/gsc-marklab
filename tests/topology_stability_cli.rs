#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn declared_single_pixel_segmentation_perturbation_is_replayed_and_quantified() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("stability.json");
    fs::write(
        &input,
        r#"{
  "base_mask":[
    [false,false,false,false,false],
    [false,false,false,false,false],
    [false,false,true,false,false],
    [false,false,false,false,false],
    [false,false,false,false,false]
  ],
  "pixel_size_um":1.0,
  "perturbation_generator":{
    "kind":"toggle_declared_pixels",
    "candidate_pixels":[[2,3]],
    "toggles_per_repetition":1
  },
  "scales_um":[0.0,1.0],
  "repetitions":1,
  "seed":99,
  "timeout_seconds":30
}"#,
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "stability",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.topology_stability");
    assert_eq!(result["seed"], 99);
    assert_eq!(result["results"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["results"][0]["toggled_pixels"],
        serde_json::json!([[2, 3]])
    );
    assert_eq!(result["results"][0]["euler_curve_linf"], 0.0);
    assert!(
        (result["results"][0]["minkowski_relative_error"]
            .as_f64()
            .unwrap()
            - 1.0)
            .abs()
            < 1e-12
    );
    assert_eq!(result["results"][0]["critical_radius_shift_um"], 1.0);
    assert!(result["results"][0]["diagram_bottleneck"].as_f64().unwrap() >= 0.0);
    assert!(result["results"][0]["landscape_l2"].as_f64().unwrap() >= 0.0);
    assert_eq!(
        result["claim_status"],
        "experimental_declared_segmentation_perturbations"
    );
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn serial_landmark_stack_recovers_section_transforms_and_propagates_posterior_uncertainty() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("stack.json");
    let base = [[0.0, 0.0], [10.0, 0.0], [0.0, 10.0], [10.0, 10.0]];
    let shifted = |dx: f64, dy: f64| {
        base.iter()
            .map(|p| [p[0] + dx, p[1] + dy])
            .collect::<Vec<_>>()
    };
    let fixture = serde_json::json!({
        "stack_id":"synthetic_serial_stack",
        "coordinate_unit":"um",
        "sections":[
            {"section_id":"s0","z_um":0.0,"observed_landmarks":shifted(1.0,-0.5)},
            {"section_id":"s1","z_um":5.0,"observed_landmarks":shifted(0.0,0.0)},
            {"section_id":"s2","z_um":10.0,"observed_landmarks":shifted(-1.0,0.75)}
        ],
        "reference_section_id":"s1",
        "landmark_noise_standard_deviation_um":0.1,
        "posterior_draws":128,
        "seed":97,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "serial-stack",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_serial_section_stack");
    assert_eq!(result["stack_posterior"]["draw_count"], 128);
    let transforms = result["stack_posterior"]["section_translations_xy_um"]
        .as_array()
        .unwrap();
    assert!((transforms[0]["posterior_mean"][0].as_f64().unwrap() + 1.0).abs() < 0.05);
    assert!((transforms[2]["posterior_mean"][0].as_f64().unwrap() - 1.0).abs() < 0.05);
    assert!(result["quality"]["landmark_rmse_um"].as_f64().unwrap() < 0.05);
    assert!(
        result["quality"]["maximum_cycle_consistency_error_um"]
            .as_f64()
            .unwrap()
            < 1e-10
    );
    assert!(
        result["downstream_uncertainty"]["stack_centroid_standard_deviation_um"][0]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_serial_stack_reconstruction"
    );
}

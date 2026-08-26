#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn landmark_lddmm_shooting_recovers_known_translation() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("lddmm.json");
    let source = vec![[0.0, 0.0], [2.0, 0.0], [0.0, 2.0], [2.0, 2.0]];
    let target = source
        .iter()
        .map(|point| [point[0] + 1.5, point[1] - 0.75])
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "source_frame":"source_slide_um",
        "target_frame":"target_slide_um",
        "source_landmarks":source,
        "target_landmarks":target,
        "kernel":"gaussian",
        "kernel_scale_um":10.0,
        "data_weight":200.0,
        "time_steps":32,
        "maximum_iterations":500,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "registration",
            "lddmm-landmarks",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.lddmm_landmark_registration");
    assert!(result["quality"]["landmark_rmse_um"].as_f64().unwrap() < 0.1);
    assert!(
        result["geodesic"]["kinetic_energy_initial"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(
        result["geodesic"]["positions"].as_array().unwrap().len(),
        33
    );
    assert_eq!(result["geodesic"]["momenta"].as_array().unwrap().len(), 33);
    assert!(
        result["diagnostics"]["objective_final"].as_f64().unwrap()
            < result["diagnostics"]["objective_initial"].as_f64().unwrap()
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_landmark_lddmm"
    );
}

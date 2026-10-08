#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn simulation_knn_threshold_flags_far_observation_and_retains_near_control() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let mut specification = serde_json::json!({
        "feature_names": ["mass", "maximum"],
        "reference_simulations": [
            {"simulation_id": "r1", "summary": [0.0, 0.0]},
            {"simulation_id": "r2", "summary": [1.0, 0.0]},
            {"simulation_id": "r3", "summary": [0.0, 1.0]},
            {"simulation_id": "r4", "summary": [1.0, 1.0]}
        ],
        "calibration_simulations": [
            {"simulation_id": "c1", "summary": [0.2, 0.2]},
            {"simulation_id": "c2", "summary": [0.8, 0.2]},
            {"simulation_id": "c3", "summary": [0.2, 0.8]},
            {"simulation_id": "c4", "summary": [0.8, 0.8]}
        ],
        "observed": {"observation_id": "obs", "summary": [10.0, 10.0]},
        "k": 2,
        "calibration_quantile": 0.75,
        "maximum_distance_visits": 1000
    });
    fs::write(&input, serde_json::to_vec(&specification).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    run(&input, &output);
    let far: serde_json::Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(far["format"], "marklab.simulation_ood");
    assert_eq!(far["status"], "out_of_support");
    assert!(far["observed_score"].as_f64().unwrap() > far["threshold"].as_f64().unwrap());
    assert_eq!(
        far["method"],
        "standardized_knn_distance_with_conformal_calibration"
    );

    specification["observed"]["summary"] = serde_json::json!([0.2, 0.2]);
    fs::write(&input, serde_json::to_vec(&specification).unwrap()).unwrap();
    let near_output = directory.path().join("near-result.json");
    run(&input, &near_output);
    let near: serde_json::Value = serde_json::from_slice(&fs::read(near_output).unwrap()).unwrap();
    assert_eq!(near["status"], "in_support");
    assert!(near["conformal_p_value"].as_f64().unwrap() > 0.0);
    assert_eq!(
        near["claim_status"],
        "simulation_support_diagnostic_not_model_validity"
    );
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "simulation-ood",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

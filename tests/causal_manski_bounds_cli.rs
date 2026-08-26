#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn bounded_outcome_ate_matches_manski_hand_interval() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "observations": [
                {"unit_id": "t1", "treatment": true, "outcome": 0.8},
                {"unit_id": "t2", "treatment": true, "outcome": 0.6},
                {"unit_id": "c1", "treatment": false, "outcome": 0.2},
                {"unit_id": "c2", "treatment": false, "outcome": 0.4}
            ],
            "outcome_lower": 0.0,
            "outcome_upper": 1.0,
            "maximum_observations": 10
        }))
        .unwrap(),
    )
    .unwrap();
    let output = directory.path().join("output.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "manski-bounds",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.manski_bounded_outcome_ate");
    assert_close(result["observed_difference"].as_f64().unwrap(), 0.4);
    assert_close(
        result["treated_potential_mean_lower"].as_f64().unwrap(),
        0.35,
    );
    assert_close(
        result["treated_potential_mean_upper"].as_f64().unwrap(),
        0.85,
    );
    assert_close(
        result["control_potential_mean_lower"].as_f64().unwrap(),
        0.15,
    );
    assert_close(
        result["control_potential_mean_upper"].as_f64().unwrap(),
        0.65,
    );
    assert_close(result["ate_lower"].as_f64().unwrap(), -0.3);
    assert_close(result["ate_upper"].as_f64().unwrap(), 0.7);
    assert_eq!(result["ate_includes_zero"], true);
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-12,
        "{actual} != {expected}"
    );
}

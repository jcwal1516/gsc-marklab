#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn binary_confounder_bias_grid_matches_hand_region() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "observed_effect": 2.0,
            "scenarios": [
                {"scenario_id": "positive_bias", "prevalence_treated": 0.8, "prevalence_control": 0.2, "outcome_effect": 1.0},
                {"scenario_id": "negative_bias", "prevalence_treated": 0.2, "prevalence_control": 0.8, "outcome_effect": 1.0},
                {"scenario_id": "reversal", "prevalence_treated": 1.0, "prevalence_control": 0.0, "outcome_effect": 3.0}
            ],
            "maximum_scenarios": 10
        }))
        .unwrap(),
    )
    .unwrap();
    let output = directory.path().join("output.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "bias-sensitivity",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.binary_confounder_bias_sensitivity"
    );
    let scenarios = result["scenarios"].as_array().unwrap();
    let positive = scenarios
        .iter()
        .find(|scenario| scenario["scenario_id"] == "positive_bias")
        .unwrap();
    assert_close(positive["bias"].as_f64().unwrap(), 0.6);
    assert_close(positive["adjusted_effect"].as_f64().unwrap(), 1.4);
    assert_close(result["region_lower"].as_f64().unwrap(), -1.0);
    assert_close(result["region_upper"].as_f64().unwrap(), 2.6);
    assert_eq!(result["region_includes_zero"], true);
    let reversal = scenarios
        .iter()
        .find(|scenario| scenario["scenario_id"] == "reversal")
        .unwrap();
    assert_eq!(reversal["sign_reversal"], true);
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-12,
        "{actual} != {expected}"
    );
}

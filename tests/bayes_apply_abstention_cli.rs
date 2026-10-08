#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn abstention_applies_frozen_uncertainty_and_ood_thresholds() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("predictions.csv");
    fs::write(
        &input,
        "prediction_id,prediction,uncertainty,ood_score\n\
p1,0.8,0.1,0.4\n\
p2,0.6,0.3,0.2\n\
p3,0.4,0.4,1.5\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "apply-abstention",
            "--input",
            input.to_str().expect("input path"),
            "--maximum-uncertainty",
            "0.25",
            "--maximum-ood-score",
            "1.0",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.abstention_decisions");
    assert_eq!(result["version"], 1);
    assert_eq!(result["policy_source"], "prespecified_validation_policy");
    assert_eq!(result["decisions"][0]["status"], "retained");
    assert_eq!(result["decisions"][0]["prediction"], 0.8);
    assert_eq!(result["decisions"][0]["reasons"], serde_json::json!([]));
    assert_eq!(result["decisions"][1]["status"], "abstained");
    assert_eq!(
        result["decisions"][1]["reasons"],
        serde_json::json!(["uncertainty_exceeds_threshold"])
    );
    assert_eq!(result["decisions"][2]["status"], "abstained");
    assert_eq!(
        result["decisions"][2]["reasons"],
        serde_json::json!([
            "uncertainty_exceeds_threshold",
            "ood_score_exceeds_threshold"
        ])
    );
}

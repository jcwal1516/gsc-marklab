#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn causal_active_suite_runs_synthetic_controls_and_retains_prospective_gap() {
    let directory = tempfile::tempdir().expect("tempdir");
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "validate-active",
            "--seed",
            "229",
            "--timeout-seconds",
            "60",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.causal_active_validation_suite");
    assert!(
        result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["status"] == "passed")
            .count()
            >= 8
    );
    assert!(result["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["status"] == "not_verified_missing_prospective_evidence"));
    assert_eq!(
        result["overall_status"],
        "partial_prospective_evidence_required"
    );
}

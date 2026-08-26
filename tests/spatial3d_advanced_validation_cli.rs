#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn advanced_3d_longitudinal_suite_runs_synthetic_controls_and_records_real_evidence_gap() {
    let directory = tempfile::tempdir().expect("tempdir");
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "validate-advanced",
            "--seed",
            "103",
            "--timeout-seconds",
            "60",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.advanced_3d_longitudinal_validation_suite"
    );
    assert_eq!(result["overall_status"], "partial_real_evidence_required");
    assert!(
        result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["status"] == "passed")
            .count()
            >= 10
    );
    assert!(result["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(
            |entry| entry["validation_id"] == "real_serial_longitudinal_clone_cohort"
                && entry["status"] == "not_verified_missing_admitted_data"
        ));
    assert_eq!(
        result["claim_status"],
        "synthetic_3d_longitudinal_validation_with_explicit_real_gap"
    );
}

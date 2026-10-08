#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn multimodal_validation_suite_runs_synthetic_controls_and_preserves_external_gaps() {
    let directory = tempfile::tempdir().expect("tempdir");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "validate",
            "--seed",
            "59",
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
        "marklab.multimodal_bayesian_validation_suite"
    );
    assert_eq!(
        result["overall_status"],
        "partial_external_evidence_required"
    );
    let entries = result["entries"].as_array().unwrap();
    assert!(
        entries
            .iter()
            .filter(|entry| entry["status"] == "passed")
            .count()
            >= 8
    );
    let spde = entries
        .iter()
        .find(|entry| entry["validation_id"] == "spde_factor_recovery")
        .unwrap();
    assert_eq!(spde["status"], "blocked_missing_mesh_owner");
    let real = entries
        .iter()
        .find(|entry| entry["validation_id"] == "patient_heldout_real_data")
        .unwrap();
    assert_eq!(real["status"], "not_verified_missing_admitted_cohort");
    let cross_inference = entries
        .iter()
        .find(|entry| entry["validation_id"] == "hmc_vi_laplace_same_model")
        .unwrap();
    assert_eq!(
        cross_inference["status"],
        "not_verified_same_model_comparison"
    );
    assert_eq!(
        result["claim_status"],
        "synthetic_validation_ledger_with_explicit_gaps"
    );
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn promotion_stops_at_first_incomplete_validation_stage() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "method_id": "synthetic_method",
            "stages": [
                {"stage": 0, "completed": true, "evidence_refs": ["oracle"], "unresolved_risks": []},
                {"stage": 1, "completed": true, "evidence_refs": ["public-benchmark"], "unresolved_risks": ["benchmark_scope"]},
                {"stage": 2, "completed": true, "evidence_refs": ["internal-controls"], "unresolved_risks": ["single_site"]},
                {"stage": 3, "completed": false, "evidence_refs": [], "unresolved_risks": ["heldout_patients_missing"]},
                {"stage": 4, "completed": false, "evidence_refs": [], "unresolved_risks": ["external_site_missing"]},
                {"stage": 5, "completed": false, "evidence_refs": [], "unresolved_risks": ["prospective_missing"]}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let output = directory.path().join("output.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "policy",
            "validation-ladder",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.real_data_validation_ladder");
    assert_eq!(result["highest_completed_stage"], 2);
    assert_eq!(
        result["highest_completed_label"],
        "internal_real_cohort_controls"
    );
    assert_eq!(
        result["promotion_blocking_risks"],
        serde_json::json!([
            "heldout_patients_missing",
            "external_site_missing",
            "prospective_missing"
        ])
    );
}

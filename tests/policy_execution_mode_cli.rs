#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn feasible_approximation_requires_explicit_approval() {
    let directory = tempfile::tempdir().unwrap();
    let denied = run(&directory, "denied", false);
    assert_eq!(denied["status"], "approximation_not_permitted");
    assert_eq!(denied["approval_required_mode"], "approx");
    assert!(denied["selected_mode"].is_null());
    assert_eq!(
        denied["assessments"][0]["rejection_reason"],
        "memory_budget_exceeded"
    );
    assert_eq!(
        denied["assessments"][1]["rejection_reason"],
        "approximation_not_approved"
    );

    let approved = run(&directory, "approved", true);
    assert_eq!(approved["status"], "selected");
    assert_eq!(approved["selected_mode"], "approx");
    assert_eq!(approved["selected_kind"], "approximate");
    assert_eq!(approved["assessments"].as_array().unwrap().len(), 2);
}

fn run(directory: &tempfile::TempDir, name: &str, approved: bool) -> serde_json::Value {
    let input = directory.path().join(format!("{name}.json"));
    let output = directory.path().join(format!("{name}-out.json"));
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "available_backends": ["cpu"],
            "data_size": 100,
            "memory_budget_bytes": 500,
            "runtime_budget_units": 1000,
            "requested_mode": null,
            "approximation_approved": approved,
            "requested_max_error": 0.2,
            "modes": [
                {"mode_id": "exact", "kind": "exact", "required_backend": "cpu", "memory_base_bytes": 100, "memory_per_item_bytes": 10, "runtime_base_units": 0, "runtime_per_item_units": 2, "validated_max_error": null},
                {"mode_id": "approx", "kind": "approximate", "required_backend": "cpu", "memory_base_bytes": 100, "memory_per_item_bytes": 2, "runtime_base_units": 0, "runtime_per_item_units": 1, "validated_max_error": 0.1}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "policy",
            "select-mode",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    serde_json::from_slice(&fs::read(output).unwrap()).unwrap()
}

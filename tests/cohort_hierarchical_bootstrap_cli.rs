#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn hierarchical_bootstrap_cli_preserves_patient_first_design() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("endpoints.csv");
    fs::write(
        &input,
        "patient_id,specimen_id,endpoint\n\
p-1,s-1,1\n\
p-1,s-2,3\n\
p-2,s-3,5\n\
p-2,s-4,7\n",
    )
    .expect("fixture");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    for output in [&first, &second] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "cohort",
                "hierarchical-bootstrap",
                "--input",
                input.to_str().expect("input path"),
                "--replicates",
                "99",
                "--seed",
                "59",
                "--alpha",
                "0.05",
                "--out",
                output.to_str().expect("output path"),
            ])
            .assert()
            .success();
    }

    let bytes = fs::read(&first).expect("first result");
    assert_eq!(bytes, fs::read(second).expect("second result"));
    let result: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_hierarchical_bootstrap");
    assert_eq!(result["version"], 1);
    assert_eq!(
        result["design"]["levels"],
        serde_json::json!(["patient", "specimen"])
    );
    assert_eq!(result["design"]["null_family"], "hierarchical_bootstrap");
    assert_eq!(
        result["design"]["permutation_unit"],
        "patient_then_nested_specimen"
    );
    assert_eq!(result["observed_mean"], 4.0);
    assert_eq!(result["patients"], 2);
    assert_eq!(result["specimens"], 4);
    assert_eq!(result["replicates"]["completed"], 99);
}

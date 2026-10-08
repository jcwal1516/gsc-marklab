#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn bootstrap_equivalence_uses_the_patient_first_interval() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("effects.csv");
    fs::write(
        &input,
        "patient_id,specimen_id,endpoint\n\
p-1,s-1,-0.10\n\
p-1,s-2,0.00\n\
p-2,s-3,0.10\n\
p-2,s-4,0.00\n\
p-3,s-5,-0.05\n\
p-3,s-6,0.05\n\
p-4,s-7,0.00\n\
p-4,s-8,0.10\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "bootstrap-equivalence",
            "--input",
            input.to_str().unwrap(),
            "--lower-margin",
            "-0.5",
            "--upper-margin",
            "0.5",
            "--alpha",
            "0.05",
            "--replicates",
            "999",
            "--seed",
            "73",
            "--margin-rationale",
            "protocol-irregular-margin-v1",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_bootstrap_equivalence");
    assert_eq!(
        result["design"]["levels"],
        serde_json::json!(["patient", "specimen"])
    );
    assert_eq!(result["interval"]["method"], "nearest_rank_percentile");
    assert_eq!(result["equivalent"], true);
    assert_eq!(result["replicates"]["completed"], 999);
    assert_eq!(result["claim_status"], "experimental_percentile_interval");
}

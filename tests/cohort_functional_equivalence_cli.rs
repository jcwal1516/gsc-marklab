#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn functional_equivalence_uses_a_simultaneous_patient_bootstrap_band() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("curves.csv");
    fs::write(
        &input,
        "patient_id,axis,difference,margin\n\
p-1,0,-0.10,0.50\n\
p-1,1,0.00,0.50\n\
p-1,2,0.10,0.50\n\
p-2,0,0.00,0.50\n\
p-2,1,0.10,0.50\n\
p-2,2,0.00,0.50\n\
p-3,0,0.10,0.50\n\
p-3,1,0.00,0.50\n\
p-3,2,-0.10,0.50\n\
p-4,0,0.00,0.50\n\
p-4,1,-0.10,0.50\n\
p-4,2,0.00,0.50\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "functional-equivalence",
            "--input",
            input.to_str().unwrap(),
            "--alpha",
            "0.05",
            "--replicates",
            "999",
            "--seed",
            "91",
            "--margin-rationale",
            "protocol-functional-margin-v1",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_functional_equivalence");
    assert_eq!(result["patient_count"], 4);
    assert_eq!(result["axis"].as_array().unwrap().len(), 3);
    assert_eq!(result["equivalent_at_all_scales"], true);
    assert_eq!(result["failing_scales"].as_array().unwrap().len(), 0);
    assert_eq!(result["replicates"]["completed"], 999);
}

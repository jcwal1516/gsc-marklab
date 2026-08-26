#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn tost_equivalence_cli_matches_student_t_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("effects.csv");
    fs::write(
        &input,
        "patient_id,effect\n\
p-1,-0.1\n\
p-2,0\n\
p-3,0.1\n\
p-4,0\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "equivalence",
            "--input",
            input.to_str().expect("input path"),
            "--lower-margin",
            "-0.2",
            "--upper-margin",
            "0.2",
            "--alpha",
            "0.05",
            "--margin-rationale",
            "protocol-margin-v1",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_equivalence");
    assert_eq!(result["version"], 1);
    assert_eq!(result["patient_count"], 4);
    assert_eq!(result["estimate"], 0.0);
    assert_eq!(result["degrees_of_freedom"], 3.0);
    assert_eq!(result["equivalent"], true);
    assert_eq!(result["margin_rationale"], "protocol-margin-v1");
    assert!(result["p_lower"].as_f64().unwrap() < 0.05);
    assert!(result["p_upper"].as_f64().unwrap() < 0.05);
    assert!(result["confidence_interval"]["lower"].as_f64().unwrap() > -0.2);
    assert!(result["confidence_interval"]["upper"].as_f64().unwrap() < 0.2);
}

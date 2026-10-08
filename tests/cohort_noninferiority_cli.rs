#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn higher_is_better_noninferiority_matches_student_t_oracle() {
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
            "noninferiority",
            "--input",
            input.to_str().expect("input path"),
            "--direction",
            "higher-is-better",
            "--margin",
            "0.2",
            "--alpha",
            "0.05",
            "--margin-rationale",
            "protocol-ni-margin-v1",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_noninferiority");
    assert_eq!(result["version"], 1);
    assert_eq!(result["direction"], "higher_is_better");
    assert_eq!(result["null_boundary"], -0.2);
    assert_eq!(result["noninferior"], true);
    assert!(result["p_value"].as_f64().unwrap() < 0.05);
    assert!(result["confidence_bound"].as_f64().unwrap() > -0.2);
}

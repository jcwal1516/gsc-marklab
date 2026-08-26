#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn linear_mmd_cli_matches_the_hand_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("fingerprints.csv");
    fs::write(
        &input,
        "patient_id,group,feature,value\n\
a-1,A,x,3\n\
a-1,A,y,0\n\
a-2,A,x,5\n\
a-2,A,y,0\n\
b-1,B,x,0\n\
b-1,B,y,0\n\
b-2,B,x,1\n\
b-2,B,y,0\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "mmd",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--kernel",
            "linear",
            "--estimator",
            "unbiased",
            "--permutations",
            "99",
            "--seed",
            "47",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_mmd");
    assert_eq!(result["version"], 1);
    assert_eq!(result["kernel"]["kind"], "linear");
    assert_eq!(result["estimator"], "unbiased");
    assert_eq!(result["mmd_squared"], 11.0);
    assert_eq!(result["permutations"]["completed"], 99);
}

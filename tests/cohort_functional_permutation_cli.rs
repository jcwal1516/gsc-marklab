#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn functional_l2_cli_matches_the_hand_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("curves.csv");
    fs::write(
        &input,
        "patient_id,group,axis,value\n\
a-1,A,0,2\n\
a-1,A,1,2\n\
a-1,A,2,2\n\
a-2,A,0,4\n\
a-2,A,1,4\n\
a-2,A,2,4\n\
b-1,B,0,0\n\
b-1,B,1,0\n\
b-1,B,2,0\n\
b-2,B,0,1\n\
b-2,B,1,1\n\
b-2,B,2,1\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "functional-permutation",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--statistic",
            "l2",
            "--permutations",
            "99",
            "--seed",
            "31",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_functional_permutation");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["axis"], serde_json::json!([0.0, 1.0, 2.0]));
    assert_eq!(
        result["observed_difference"],
        serde_json::json!([2.5, 2.5, 2.5])
    );
    assert_eq!(result["observed_statistic"], 12.5);
    assert_eq!(result["permutations"]["completed"], 99);
}

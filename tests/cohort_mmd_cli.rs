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
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["design"]["blocked"], false);
    assert!(result["design"].get("null_family").is_none());
    assert!(result["design"].get("block_count").is_none());
    assert_eq!(result["kernel"]["kind"], "linear");
    assert_eq!(result["estimator"], "unbiased");
    assert_eq!(result["mmd_squared"], 11.0);
    assert_eq!(result["permutations"]["completed"], 99);
}

#[test]
fn mmd_cli_rejects_conflicting_blocks_within_one_patient() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("conflicting-blocks.csv");
    fs::write(
        &input,
        "patient_id,group,feature,value,block\n\
a-1,A,x,3,north\n\
a-1,A,y,1,south\n\
a-2,A,x,5,north\n\
a-2,A,y,2,north\n\
b-1,B,x,0,north\n\
b-1,B,y,0,north\n\
b-2,B,x,1,north\n\
b-2,B,y,1,north\n",
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
            "19",
            "--seed",
            "47",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "patient a-1 has conflicting fingerprint blocks",
        ));
}

#[test]
fn linear_mmd_cli_accepts_complete_patient_blocks_and_reports_the_design() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("blocked-fingerprints.csv");
    fs::write(
        &input,
        "patient_id,group,feature,value,block\n\
a-1,A,x,3,north\n\
a-2,A,x,5,north\n\
b-1,B,x,0,north\n\
b-2,B,x,1,north\n\
a-3,A,x,4,south\n\
a-4,A,x,6,south\n\
b-3,B,x,1,south\n\
b-4,B,x,2,south\n",
    )
    .expect("fixture");
    let output = directory.path().join("blocked-result.json");

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
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["design"]["null_family"], "population_independence");
    assert_eq!(result["design"]["blocked"], true);
    assert_eq!(result["design"]["block_count"], 2);
    assert_eq!(result["groups"]["group_a_patients"], 4);
    assert_eq!(result["groups"]["group_b_patients"], 4);
    assert_eq!(result["permutations"]["completed"], 99);
}

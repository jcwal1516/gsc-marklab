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
    assert_eq!(result["design"]["blocked"], false);
    assert!(result["design"].get("null_family").is_none());
    assert!(result["design"].get("block_count").is_none());
    assert_eq!(result["axis"], serde_json::json!([0.0, 1.0, 2.0]));
    assert_eq!(
        result["observed_difference"],
        serde_json::json!([2.5, 2.5, 2.5])
    );
    assert_eq!(result["observed_statistic"], 12.5);
    assert_eq!(result["permutations"]["completed"], 99);
}

#[test]
fn functional_l2_cli_accepts_complete_patient_blocks_and_reports_the_design() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("blocked-curves.csv");
    fs::write(
        &input,
        "patient_id,group,axis,value,block\n\
a-1,A,0,2,north\n\
a-1,A,1,3,north\n\
a-2,A,0,4,north\n\
a-2,A,1,5,north\n\
b-1,B,0,0,north\n\
b-1,B,1,1,north\n\
b-2,B,0,1,north\n\
b-2,B,1,2,north\n\
a-3,A,0,3,south\n\
a-3,A,1,4,south\n\
a-4,A,0,5,south\n\
a-4,A,1,6,south\n\
b-3,B,0,1,south\n\
b-3,B,1,2,south\n\
b-4,B,0,2,south\n\
b-4,B,1,3,south\n",
    )
    .expect("fixture");
    let output = directory.path().join("blocked-result.json");

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
    assert_eq!(result["design"]["null_family"], "population_independence");
    assert_eq!(result["design"]["blocked"], true);
    assert_eq!(result["design"]["block_count"], 2);
    assert_eq!(result["groups"]["group_a_patients"], 4);
    assert_eq!(result["groups"]["group_b_patients"], 4);
    assert_eq!(result["permutations"]["completed"], 99);
}

#[test]
fn functional_cli_rejects_conflicting_blocks_within_one_curve() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("conflicting-blocks.csv");
    fs::write(
        &input,
        "patient_id,group,axis,value,block\n\
a-1,A,0,2,north\n\
a-1,A,1,3,south\n\
a-2,A,0,4,north\n\
a-2,A,1,5,north\n\
b-1,B,0,0,north\n\
b-1,B,1,1,north\n\
b-2,B,0,1,north\n\
b-2,B,1,2,north\n",
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
            "19",
            "--seed",
            "31",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "patient a-1 has conflicting functional blocks",
        ));
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn euclidean_energy_cli_matches_the_hand_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("fingerprints.csv");
    fs::write(
        &input,
        "patient_id,group,feature,value\n\
a-1,A,x,3\n\
a-2,A,x,5\n\
b-1,B,x,0\n\
b-2,B,x,1\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "energy",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--metric",
            "euclidean",
            "--permutations",
            "99",
            "--seed",
            "53",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_energy");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["design"]["blocked"], false);
    assert!(result["design"].get("null_family").is_none());
    assert!(result["design"].get("block_count").is_none());
    assert_eq!(result["metric"], "euclidean");
    assert_eq!(result["energy_distance"], 5.5);
    assert_eq!(result["permutations"]["completed"], 99);
}

#[test]
fn euclidean_energy_cli_accepts_complete_patient_blocks_and_reports_the_design() {
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
            "energy",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--metric",
            "euclidean",
            "--permutations",
            "99",
            "--seed",
            "53",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_energy");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["design"]["null_family"], "population_independence");
    assert_eq!(result["design"]["blocked"], true);
    assert_eq!(result["design"]["block_count"], 2);
    assert_eq!(result["groups"]["group_a_patients"], 4);
    assert_eq!(result["groups"]["group_b_patients"], 4);
    assert_eq!(result["permutations"]["completed"], 99);
}

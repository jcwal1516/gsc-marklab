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
    assert_eq!(result["metric"], "euclidean");
    assert_eq!(result["energy_distance"], 5.5);
    assert_eq!(result["permutations"]["completed"], 99);
}

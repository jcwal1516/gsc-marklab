#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn patient_level_cli_matches_the_hand_oracle_and_is_byte_deterministic() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("endpoints.csv");
    fs::write(
        &input,
        "patient_id,group,endpoint,block\n\
a-1,A,8,\n\
a-2,A,9,\n\
b-1,B,1,\n\
b-2,B,2,\n",
    )
    .expect("fixture");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    for output in [&first, &second] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "cohort",
                "permutation",
                "--input",
                input.to_str().expect("input path"),
                "--group-a",
                "A",
                "--group-b",
                "B",
                "--permutations",
                "99",
                "--seed",
                "17",
                "--alternative",
                "less",
                "--out",
                output.to_str().expect("output path"),
            ])
            .assert()
            .success();
    }

    let first_bytes = fs::read(&first).expect("first result");
    assert_eq!(first_bytes, fs::read(&second).expect("second result"));
    assert_eq!(first_bytes.last(), Some(&b'\n'));
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).expect("result JSON");
    assert_eq!(result["format"], "marklab.cohort_permutation");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["design"]["blocked"], false);
    assert_eq!(result["groups"]["group_a"]["patient_count"], 2);
    assert_eq!(result["groups"]["group_b"]["patient_count"], 2);
    assert_eq!(result["groups"]["group_a"]["mean"], 8.5);
    assert_eq!(result["groups"]["group_b"]["mean"], 1.5);
    assert_eq!(result["effect_group_a_minus_group_b"], 7.0);
    assert!(
        (result["studentized_statistic"].as_f64().expect("statistic") - 9.899_494_936_611_665)
            .abs()
            < 1e-12
    );
    assert_eq!(result["p_value"], 1.0);
    assert_eq!(result["permutations"]["requested"], 99);
    assert_eq!(result["permutations"]["attempted"], 99);
    assert_eq!(result["permutations"]["completed"], 99);
}

#[test]
fn duplicate_patient_is_rejected_before_result_publication() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("endpoints.csv");
    let output = directory.path().join("result.json");
    fs::write(
        &input,
        "patient_id,group,endpoint\n\
same,A,1\n\
same,B,2\n\
a-2,A,3\n\
b-2,B,4\n",
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "permutation",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "19",
            "--seed",
            "17",
            "--alternative",
            "two-sided",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate patient_id"));
    assert!(!output.exists());
}

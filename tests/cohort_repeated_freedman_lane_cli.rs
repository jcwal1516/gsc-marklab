#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn repeated_freedman_lane_recovers_the_common_within_subject_slope() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("repeated.csv");
    fs::write(
        &input,
        "subject_id,visit_id,outcome,target\n\
s-1,v-1,8.2,-1\n\
s-1,v-2,10.0,0\n\
s-1,v-3,11.8,1\n\
s-2,v-1,18.0,-1\n\
s-2,v-2,20.0,0\n\
s-2,v-3,22.0,1\n\
s-3,v-1,27.8,-1\n\
s-3,v-2,30.0,0\n\
s-3,v-3,32.2,1\n\
s-4,v-1,37.6,-1\n\
s-4,v-2,40.0,0\n\
s-4,v-3,42.4,1\n",
    )
    .expect("fixture");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    for output in [&first, &second] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "cohort",
                "repeated-freedman-lane",
                "--input",
                input.to_str().expect("input path"),
                "--permutations",
                "99",
                "--seed",
                "20260825",
                "--out",
                output.to_str().expect("output path"),
            ])
            .assert()
            .success();
    }

    let bytes = fs::read(&first).expect("first output");
    assert_eq!(bytes, fs::read(&second).expect("second output"));
    let result: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_repeated_freedman_lane");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["subject_count"], 4);
    assert_eq!(result["design"]["row_count"], 12);
    assert_eq!(
        result["design"]["residual_randomization"],
        "whole_subject_sign_flip"
    );
    assert!((result["target_coefficient"].as_f64().unwrap() - 2.1).abs() < 1e-12);
    assert!(result["studentized_statistic"].as_f64().unwrap() > 10.0);
    assert_eq!(result["permutations"]["completed"], 99);
    assert_eq!(
        result["claim_status"],
        "experimental_residual_exchangeability_required"
    );
}

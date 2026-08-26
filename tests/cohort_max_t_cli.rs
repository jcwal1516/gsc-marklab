#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn max_t_cli_matches_observed_hand_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("endpoints.csv");
    fs::write(
        &input,
        "patient_id,group,endpoint,value\n\
a-1,A,e1,8\n\
a-1,A,e2,4\n\
a-2,A,e1,9\n\
a-2,A,e2,7\n\
b-1,B,e1,1\n\
b-1,B,e2,2\n\
b-2,B,e1,2\n\
b-2,B,e2,3\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "max-t",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "99",
            "--seed",
            "41",
            "--alpha",
            "0.05",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_max_t");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["endpoints"][0]["endpoint"], "e1");
    assert_eq!(result["endpoints"][0]["effect_group_a_minus_group_b"], 7.0);
    assert!(
        (result["endpoints"][0]["studentized_statistic"]
            .as_f64()
            .expect("e1 statistic")
            - 9.899_494_936_611_665)
            .abs()
            < 1e-12
    );
    assert_eq!(result["endpoints"][1]["endpoint"], "e2");
    assert_eq!(result["endpoints"][1]["effect_group_a_minus_group_b"], 3.0);
    assert!(
        (result["endpoints"][1]["studentized_statistic"]
            .as_f64()
            .expect("e2 statistic")
            - 1.897_366_596_101_027_5)
            .abs()
            < 1e-12
    );
    assert_eq!(result["permutations"]["completed"], 99);
}

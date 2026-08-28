#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn paired_cli_matches_the_hand_oracle_and_is_byte_deterministic() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("paired.csv");
    fs::write(
        &input,
        "patient_id,condition,endpoint\n\
p-1,A,1\n\
p-1,B,3\n\
p-2,A,2\n\
p-2,B,5\n\
p-3,A,3\n\
p-3,B,7\n\
p-4,A,4\n\
p-4,B,9\n",
    )
    .expect("fixture");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    for output in [&first, &second] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "cohort",
                "paired-permutation",
                "--input",
                input.to_str().expect("input path"),
                "--condition-a",
                "A",
                "--condition-b",
                "B",
                "--permutations",
                "99",
                "--seed",
                "23",
                "--alternative",
                "greater",
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
    assert_eq!(result["format"], "marklab.cohort_paired_permutation");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient_pair");
    assert_eq!(result["design"]["null_family"], "paired_sign_flip");
    assert_eq!(
        result["design"]["permutation_unit"],
        "complete_patient_pair_difference"
    );
    assert_eq!(result["pairs"]["completed"], 4);
    assert_eq!(result["conditions"]["condition_a"]["mean"], 2.5);
    assert_eq!(result["conditions"]["condition_b"]["mean"], 6.0);
    assert_eq!(result["effect_condition_b_minus_condition_a"], 3.5);
    assert!(
        (result["studentized_statistic"].as_f64().expect("statistic") - 5.422_176_684_690_384)
            .abs()
            < 1e-12
    );
    assert_eq!(result["permutations"]["completed"], 99);
}

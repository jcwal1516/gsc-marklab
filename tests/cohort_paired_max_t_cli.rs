#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn paired_max_t_cli_moves_each_complete_patient_difference_vector_as_one_unit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("paired-endpoints.csv");
    let mut rows = String::from("patient_id,condition,endpoint,value\n");
    for (patient, a, b) in [
        ("p-1", [1.0, 4.0, 2.0], [3.0, 5.0, 2.5]),
        ("p-2", [2.0, 3.0, 5.0], [5.0, 5.0, 5.5]),
        ("p-3", [3.0, 2.0, 3.0], [7.0, 5.0, 4.0]),
        ("p-4", [4.0, 1.0, 4.0], [9.0, 5.0, 4.5]),
    ] {
        for (endpoint, a_value, b_value) in [
            ("strong", a[0], b[0]),
            ("middle", a[1], b[1]),
            ("weak", a[2], b[2]),
        ] {
            rows.push_str(&format!("{patient},A,{endpoint},{a_value}\n"));
            rows.push_str(&format!("{patient},B,{endpoint},{b_value}\n"));
        }
    }
    fs::write(&input, rows).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "paired-max-t",
            "--input",
            input.to_str().expect("input path"),
            "--condition-a",
            "A",
            "--condition-b",
            "B",
            "--permutations",
            "199",
            "--seed",
            "23",
            "--alpha",
            "0.05",
            "--step-down",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_paired_max_t");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient_pair");
    assert_eq!(result["design"]["null_family"], "paired_sign_flip");
    assert_eq!(
        result["design"]["permutation_unit"],
        "complete_patient_pair_difference_vector"
    );
    assert_eq!(
        result["design"]["multiplicity"],
        "complete_endpoint_family_max_t"
    );
    assert_eq!(result["design"]["correction"], "step_down_max_t");
    assert_eq!(result["pairs"]["completed"], 4);
    assert_eq!(result["conditions"]["condition_a"], "A");
    assert_eq!(result["conditions"]["condition_b"], "B");
    assert_eq!(result["endpoints"][0]["endpoint"], "middle");
    assert_eq!(
        result["endpoints"][0]["effect_condition_b_minus_condition_a"],
        2.5
    );
    assert_eq!(result["endpoints"][1]["endpoint"], "strong");
    assert_eq!(
        result["endpoints"][1]["effect_condition_b_minus_condition_a"],
        3.5
    );
    assert_eq!(result["endpoints"][2]["endpoint"], "weak");
    assert_eq!(
        result["endpoints"][2]["effect_condition_b_minus_condition_a"],
        0.625
    );
    assert_eq!(result["permutations"]["completed"], 199);
}

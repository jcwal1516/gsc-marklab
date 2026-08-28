#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn covariate_permutation_cli_adjusts_one_prespecified_patient_covariate() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    fs::write(
        &input,
        "patient_id,group,outcome,covariate\n\
a-1,A,2.0,0\n\
a-2,A,5.5,1\n\
a-3,A,6.0,2\n\
a-4,A,10.0,3\n\
a-5,A,10.5,4\n\
a-6,A,14.0,5\n\
b-1,B,-1.0,0\n\
b-2,B,2.5,1\n\
b-3,B,3.0,2\n\
b-4,B,7.0,3\n\
b-5,B,7.5,4\n\
b-6,B,11.0,5\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "covariate-permutation",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "199",
            "--seed",
            "47",
            "--alternative",
            "two-sided",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_covariate_permutation");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient_residual");
    assert_eq!(
        result["design"]["null_family"],
        "covariate_conditional_residual_permutation"
    );
    assert_eq!(
        result["design"]["permutation_unit"],
        "complete_patient_residual"
    );
    assert_eq!(result["design"]["reduced_model_columns"], 2);
    assert_eq!(result["design"]["full_model_columns"], 3);
    assert_eq!(result["design"]["covariate_center"], 2.5);
    assert_eq!(result["design"]["covariate_scale"], 2.5);
    assert_eq!(result["patients"]["total"], 12);
    assert_eq!(result["groups"]["group_a_patients"], 6);
    assert_eq!(result["groups"]["group_b_patients"], 6);
    assert!(
        (result["effect_group_a_minus_group_b"]
            .as_f64()
            .expect("effect")
            - 3.0)
            .abs()
            < 1e-12
    );
    assert_eq!(result["permutations"]["completed"], 199);
}

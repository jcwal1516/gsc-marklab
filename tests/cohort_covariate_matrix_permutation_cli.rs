#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn covariate_matrix_cli_adjusts_an_exact_named_nuisance_matrix() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let mut rows = String::from("patient_id,group,outcome,covariate,value\n");
    let noise = [-1.0, 0.5, -1.0, 1.0, -0.5, 1.0];
    for group in ["A", "B"] {
        for (index, noise) in noise.into_iter().enumerate() {
            let batch = (index % 2) as f64;
            let outcome =
                2.0 * index as f64 + 0.5 * batch + noise + if group == "A" { 3.0 } else { 0.0 };
            let patient = format!("{}-{}", group.to_ascii_lowercase(), index + 1);
            rows.push_str(&format!("{patient},{group},{outcome},age,{index}\n"));
            rows.push_str(&format!("{patient},{group},{outcome},batch,{batch}\n"));
        }
    }
    fs::write(&input, rows).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "covariate-matrix-permutation",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "199",
            "--seed",
            "53",
            "--alternative",
            "two-sided",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.cohort_covariate_matrix_permutation"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["nuisance_columns"], 2);
    assert_eq!(result["design"]["reduced_model_columns"], 3);
    assert_eq!(result["design"]["full_model_columns"], 4);
    assert_eq!(
        result["covariates"]["names"],
        serde_json::json!(["age", "batch"])
    );
    assert_eq!(
        result["covariates"]["centers"],
        serde_json::json!([2.5, 0.5])
    );
    assert_eq!(
        result["covariates"]["scales"],
        serde_json::json!([2.5, 0.5])
    );
    assert_eq!(result["patients"]["total"], 12);
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

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
    assert_eq!(result["version"], 2);
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
    assert_eq!(result["permutations"]["requested"], 199);
}

#[test]
fn blocked_covariate_matrix_cli_reports_the_within_block_contrast() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("blocked-patients.csv");
    // Group A is concentrated in the high-outcome block; within each block the groups do not differ.
    let fixture = [
        ("north", "A", 1, 11.5),
        ("north", "A", 2, 10.0),
        ("north", "A", 3, 12.0),
        ("north", "A", 4, 11.5),
        ("north", "B", 2, 12.0),
        ("north", "B", 3, 10.5),
        ("south", "A", 1, 1.0),
        ("south", "A", 4, 1.5),
        ("south", "B", 1, 1.5),
        ("south", "B", 2, 0.0),
        ("south", "B", 3, 2.0),
        ("south", "B", 4, 1.5),
    ];
    let mut rows = String::from("patient_id,group,outcome,covariate,value,block\n");
    for (index, (block, group, age, outcome)) in fixture.into_iter().enumerate() {
        rows.push_str(&format!(
            "p-{index:02},{group},{outcome},age,{age},{block}\n"
        ));
        rows.push_str(&format!(
            "p-{index:02},{group},{outcome},batch,{},{block}\n",
            age % 2
        ));
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
    assert_eq!(result["version"], 2);
    assert_eq!(result["design"]["blocked"], true);
    assert_eq!(result["design"]["block_count"], 2);
    assert_eq!(result["design"]["block_indicator_columns"], 1);
    assert_eq!(result["design"]["reduced_model_columns"], 4);
    assert_eq!(result["design"]["residual_degrees_of_freedom"], 7);
    assert!(
        result["effect_group_a_minus_group_b"]
            .as_f64()
            .expect("effect")
            .abs()
            < 1e-9
    );
    assert!(result["p_value"].as_f64().expect("p") > 0.5);
}

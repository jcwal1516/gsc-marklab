#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn cluster_covariate_cli_adjusts_equal_weight_cluster_summaries() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("cluster_covariates.csv");
    fs::write(
        &input,
        "patient_id,cluster_id,group,outcome,covariate,value\n\
a1-1,a1,A,4.0,age,-3\n\
a1-1,a1,A,4.0,batch,-1\n\
a1-2,a1,A,6.0,age,-1\n\
a1-2,a1,A,6.0,batch,1\n\
a2,a2,A,5.0,age,-1\n\
a2,a2,A,5.0,batch,-1\n\
a3,a3,A,7.0,age,1\n\
a3,a3,A,7.0,batch,1\n\
a4,a4,A,7.0,age,3\n\
a4,a4,A,7.0,batch,-1\n\
b1,b1,B,0.0,age,-3\n\
b1,b1,B,0.0,batch,1\n\
b2,b2,B,2.0,age,-1\n\
b2,b2,B,2.0,batch,-1\n\
b3,b3,B,2.0,age,1\n\
b3,b3,B,2.0,batch,1\n\
b4-1,b4,B,3.0,age,2\n\
b4-1,b4,B,3.0,batch,-1\n\
b4-2,b4,B,5.0,age,4\n\
b4-2,b4,B,5.0,batch,1\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "cluster-covariate-permutation",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "99",
            "--seed",
            "71",
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
        "marklab.cohort_cluster_covariate_permutation"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["analysis_level"], "cluster");
    assert_eq!(
        result["design"]["null_family"],
        "cluster_covariate_residual_permutation"
    );
    assert_eq!(
        result["design"]["permutation_unit"],
        "complete_cluster_residual"
    );
    assert_eq!(
        result["design"]["cluster_summary"],
        "equal_weight_patient_mean"
    );
    assert_eq!(result["patients"], 10);
    assert_eq!(result["clusters"]["total"], 8);
    assert_eq!(result["clusters"]["group_a"], 4);
    assert_eq!(result["clusters"]["group_b"], 4);
    assert_eq!(
        result["covariates"]["names"],
        serde_json::json!(["age", "batch"])
    );
    assert_eq!(result["design"]["nuisance_columns"], 2);
    assert_eq!(result["design"]["reduced_model_columns"], 3);
    assert_eq!(result["design"]["full_model_columns"], 4);
    assert_eq!(result["design"]["residual_degrees_of_freedom"], 4);
    assert_eq!(result["permutations"]["completed"], 99);
    assert!(result["effect_group_a_minus_group_b"].as_f64().is_some());
    assert!(result["p_value"].as_f64().is_some());
}

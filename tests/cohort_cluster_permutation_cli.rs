#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn cluster_cli_uses_equal_weight_whole_cluster_endpoints() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("clusters.csv");
    fs::write(
        &input,
        "patient_id,cluster_id,group,endpoint\n\
a-1,cluster-a1,A,1\n\
a-2,cluster-a1,A,3\n\
a-3,cluster-a2,A,4\n\
a-4,cluster-a3,A,5\n\
a-5,cluster-a3,A,7\n\
a-6,cluster-a3,A,9\n\
b-1,cluster-b1,B,0\n\
b-2,cluster-b1,B,2\n\
b-3,cluster-b2,B,2\n\
b-4,cluster-b3,B,1\n\
b-5,cluster-b3,B,3\n\
b-6,cluster-b3,B,5\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "cluster-permutation",
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
    assert_eq!(result["format"], "marklab.cohort_cluster_permutation");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["analysis_level"], "cluster");
    assert_eq!(result["design"]["null_family"], "cluster_label_permutation");
    assert_eq!(
        result["design"]["permutation_unit"],
        "complete_cluster_endpoint"
    );
    assert_eq!(result["patients"], 12);
    assert_eq!(result["clusters"]["total"], 6);
    assert_eq!(result["clusters"]["group_a"], 3);
    assert_eq!(result["clusters"]["group_b"], 3);
    assert!(
        (result["effect_group_a_minus_group_b"]
            .as_f64()
            .expect("effect")
            - 7.0 / 3.0)
            .abs()
            < 1e-14
    );
    assert_eq!(result["permutations"]["completed"], 99);
}

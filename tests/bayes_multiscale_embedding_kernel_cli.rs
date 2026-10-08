#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn multiscale_kernel_matches_weighted_linear_hand_oracle_and_sensitivity() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("summaries.csv");
    let weights = directory.path().join("weights.csv");
    let output = directory.path().join("output.json");
    fs::write(
        &input,
        "sample_id,scale_um,embedding_0,embedding_1\na,10,1,0\na,20,0,2\nb,10,3,0\nb,20,0,1\n",
    )
    .unwrap();
    fs::write(&weights, "scale_um,weight\n10,0.25\n20,0.75\n").unwrap();

    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "multiscale-embedding-kernel",
            "--input",
            input.to_str().unwrap(),
            "--weights",
            weights.to_str().unwrap(),
            "--sample-a",
            "a",
            "--sample-b",
            "b",
            "--base-kernel",
            "linear",
            "--maximum-component-scale-visits",
            "4",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.multiscale_embedding_kernel");
    assert_eq!(result["version"], 1);
    assert_eq!(result["base_kernel"], "linear");
    assert_eq!(result["total"], 2.25);
    assert_eq!(result["scales"][0]["raw_kernel"], 3.0);
    assert_eq!(result["scales"][0]["contribution"], 0.75);
    assert_eq!(result["scales"][1]["raw_kernel"], 2.0);
    assert_eq!(result["scales"][1]["contribution"], 1.5);
    assert_eq!(
        result["drop_one_scale_sensitivity"][0]["renormalized_total"],
        2.0
    );
    assert_eq!(
        result["drop_one_scale_sensitivity"][1]["renormalized_total"],
        3.0
    );
}

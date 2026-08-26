#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn shrinkage_mahalanobis_uses_training_fit_and_validation_threshold() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("representations.csv");
    fs::write(
        &input,
        "unit_id,split,domain,embedding_1,embedding_2\n\
t1,train,site-a,-1,-1\n\
t2,train,site-a,-1,1\n\
t3,train,site-a,1,-1\n\
t4,train,site-a,1,1\n\
v1,validation,site-b,0,0\n\
v2,validation,site-b,1,0\n\
v3,validation,site-b,0,2\n\
q1,test,site-c,0.5,0.5\n\
q2,test,site-c,2,2\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "ood-score",
            "--input",
            input.to_str().expect("input path"),
            "--shrinkage",
            "0.25",
            "--validation-quantile",
            "0.6666666666666666",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.ood_score");
    assert_eq!(result["method"], "mahalanobis_shrinkage");
    assert_eq!(result["fit_split"], "train");
    assert_eq!(result["threshold_split"], "validation");
    assert_eq!(result["threshold"], 1.0);
    assert!((result["scores"][0]["score"].as_f64().unwrap() - 0.5_f64.sqrt()).abs() < 1e-12);
    assert_eq!(result["scores"][0]["exceeds_threshold"], false);
    assert!((result["scores"][1]["score"].as_f64().unwrap() - 8.0_f64.sqrt()).abs() < 1e-12);
    assert_eq!(result["scores"][1]["exceeds_threshold"], true);
}

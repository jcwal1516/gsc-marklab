#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn region_retrieval_builds_training_only_exact_index_and_explains_distance() {
    let directory = tempfile::tempdir().unwrap();
    let training = directory.path().join("training.csv");
    let query = directory.path().join("query.csv");
    let output = directory.path().join("output.json");
    let provenance = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fs::write(&training, format!("region_id,patient_id,site_id,split,domain,provenance_sha256,embedding_0,embedding_1\nr1,p1,s1,train,tumor,{provenance},0,0\nr2,p2,s1,train,tumor,{provenance},1,0\nr3,p3,s2,train,tumor,{provenance},3,4\nr4,p4,s2,train,tumor,{provenance},5,4\n")).unwrap();
    fs::write(&query, format!("region_id,patient_id,site_id,domain,provenance_sha256,embedding_0,embedding_1\nq,p9,s3,tumor,{provenance},0.9,0\n")).unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "retrieve-analogous-regions",
            "--training",
            training.to_str().unwrap(),
            "--query",
            query.to_str().unwrap(),
            "--k",
            "2",
            "--leakage-policy",
            "exclude_same_patient",
            "--maximum-component-candidate-visits",
            "32",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.region_retrieval");
    assert_eq!(result["index"]["search"], "exact");
    assert_eq!(result["index"]["approximation_recall_against_exact"], 1.0);
    assert_eq!(result["matches"][0]["region_id"], "r2");
    assert_eq!(result["matches"][1]["region_id"], "r1");
    for matched in result["matches"].as_array().unwrap() {
        let sum = matched["component_squared_contributions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .sum::<f64>();
        assert!((sum - matched["distance"].as_f64().unwrap().powi(2)).abs() < 1e-12);
    }
    assert!(result["ood_score"].as_f64().unwrap().is_finite());
    assert_eq!(
        result["claim_status"],
        "analogous_under_frozen_metric_not_biologically_identical"
    );
}

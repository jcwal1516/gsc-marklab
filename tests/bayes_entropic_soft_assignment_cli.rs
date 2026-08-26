#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn dustbin_assignment_prefers_unmatched_over_large_real_cost() {
    let directory = tempfile::tempdir().expect("tempdir");
    let source = directory.path().join("source.csv");
    let target = directory.path().join("target.csv");
    let cost = directory.path().join("cost.csv");
    fs::write(&source, "source_id,mass\ns1,1\n").expect("source");
    fs::write(&target, "target_id,mass\nt1,1\n").expect("target");
    fs::write(&cost, "source_id,target_id,cost\ns1,t1,10\n").expect("cost");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "entropic-soft-assignment",
            "--source",
            source.to_str().expect("source path"),
            "--target",
            target.to_str().expect("target path"),
            "--cost",
            cost.to_str().expect("cost path"),
            "--epsilon",
            "0.1",
            "--dustbin-cost",
            "0",
            "--tolerance",
            "1e-12",
            "--maximum-iterations",
            "1000",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.entropic_soft_assignment");
    assert!(result["real_plan"][0]["mass"].as_f64().unwrap() < 0.01);
    assert!(result["unmatched_source_mass"][0].as_f64().unwrap() > 0.99);
    assert!(result["unmatched_target_mass"][0].as_f64().unwrap() > 0.99);
    assert_eq!(result["epsilon_sensitivity"].as_array().unwrap().len(), 3);
    assert_eq!(
        result["claim_status"],
        "probabilistic_compatibility_not_cell_identity"
    );
}

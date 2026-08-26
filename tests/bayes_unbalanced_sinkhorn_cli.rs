#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn unbalanced_sinkhorn_matches_the_one_cell_fixed_point() {
    let directory = tempfile::tempdir().expect("tempdir");
    let source = directory.path().join("source.csv");
    let target = directory.path().join("target.csv");
    let cost = directory.path().join("cost.csv");
    fs::write(&source, "source_id,mass\ns1,2\n").expect("source");
    fs::write(&target, "target_id,mass\nt1,8\n").expect("target");
    fs::write(&cost, "source_id,target_id,cost\ns1,t1,0\n").expect("cost");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "unbalanced-sinkhorn",
            "--source",
            source.to_str().expect("source path"),
            "--target",
            target.to_str().expect("target path"),
            "--cost",
            cost.to_str().expect("cost path"),
            "--epsilon",
            "1",
            "--tau-source",
            "1",
            "--tau-target",
            "1",
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
    assert_eq!(result["format"], "marklab.unbalanced_sinkhorn");
    assert_eq!(result["converged"], true);
    let expected = 16.0_f64.powf(1.0 / 3.0);
    assert!((result["transported_mass"].as_f64().unwrap() - expected).abs() < 1e-10);
    assert!((result["plan"][0]["mass"].as_f64().unwrap() - expected).abs() < 1e-10);
    assert!((result["source_marginals"][0].as_f64().unwrap() - expected).abs() < 1e-10);
    assert!((result["target_marginals"][0].as_f64().unwrap() - expected).abs() < 1e-10);
    assert!(result["source_kl_divergence"].as_f64().unwrap() > 0.0);
    assert!(result["target_kl_divergence"].as_f64().unwrap() > 0.0);
}

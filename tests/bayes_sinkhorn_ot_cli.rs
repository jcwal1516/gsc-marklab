#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn balanced_sinkhorn_matches_the_symmetric_closed_form() {
    let directory = tempfile::tempdir().expect("tempdir");
    let source = directory.path().join("source.csv");
    let target = directory.path().join("target.csv");
    let cost = directory.path().join("cost.csv");
    fs::write(&source, "source_id,mass\ns1,0.5\ns2,0.5\n").expect("source");
    fs::write(&target, "target_id,mass\nt1,0.5\nt2,0.5\n").expect("target");
    fs::write(
        &cost,
        "source_id,target_id,cost\ns1,t1,0\ns1,t2,1\ns2,t1,1\ns2,t2,0\n",
    )
    .expect("cost");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "sinkhorn-ot",
            "--source",
            source.to_str().expect("source path"),
            "--target",
            target.to_str().expect("target path"),
            "--cost",
            cost.to_str().expect("cost path"),
            "--epsilon",
            "1",
            "--tolerance",
            "1e-12",
            "--maximum-iterations",
            "100",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.sinkhorn_ot");
    assert_eq!(result["converged"], true);
    let diagonal = 0.5 / (1.0 + (-1.0_f64).exp());
    let off_diagonal = 0.5 - diagonal;
    let plan = result["plan"].as_array().unwrap();
    assert!((plan[0]["mass"].as_f64().unwrap() - diagonal).abs() < 1e-12);
    assert!((plan[1]["mass"].as_f64().unwrap() - off_diagonal).abs() < 1e-12);
    assert!((plan[2]["mass"].as_f64().unwrap() - off_diagonal).abs() < 1e-12);
    assert!((plan[3]["mass"].as_f64().unwrap() - diagonal).abs() < 1e-12);
    assert!(result["maximum_marginal_residual"].as_f64().unwrap() <= 1e-12);
    for marginal in result["source_marginals"]
        .as_array()
        .unwrap()
        .iter()
        .chain(result["target_marginals"].as_array().unwrap())
    {
        assert!((marginal.as_f64().unwrap() - 0.5).abs() < 1e-12);
    }
}

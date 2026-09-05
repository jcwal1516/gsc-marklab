#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn partial_ot_matches_the_forced_one_cell_plan() {
    let directory = tempfile::tempdir().expect("tempdir");
    let source = directory.path().join("source.csv");
    let target = directory.path().join("target.csv");
    let cost = directory.path().join("cost.csv");
    fs::write(&source, "source_id,mass\ns1,2\n").expect("source");
    fs::write(&target, "target_id,mass\nt1,3\n").expect("target");
    fs::write(&cost, "source_id,target_id,cost\ns1,t1,4\n").expect("cost");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "partial-ot",
            "--source",
            source.to_str().expect("source path"),
            "--target",
            target.to_str().expect("target path"),
            "--cost",
            cost.to_str().expect("cost path"),
            "--transported-mass",
            "1.5",
            "--epsilon",
            "0.5",
            "--timeout-seconds",
            "30",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.partial_ot");
    assert_eq!(result["backend"]["version"], "1.18.1");
    assert!((result["transported_mass"].as_f64().unwrap() - 1.5).abs() < 1e-10);
    assert!((result["plan"][0]["mass"].as_f64().unwrap() - 1.5).abs() < 1e-10);
    assert!((result["unmatched_source_mass"][0].as_f64().unwrap() - 0.5).abs() < 1e-10);
    assert!((result["unmatched_target_mass"][0].as_f64().unwrap() - 1.5).abs() < 1e-10);
    assert!((result["transport_cost"].as_f64().unwrap() - 6.0).abs() < 1e-10);
    assert_eq!(result["constraint_status"], "feasible_within_tolerance");
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn annealed_smc_matches_conjugate_posterior_and_evidence() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(&input, "observation\n1\n2\n3\n4\n").expect("input");
    let output = directory.path().join("smc.json");
    let repeated_output = directory.path().join("smc-repeated.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "normal-mean-smc",
            "--input",
            input.to_str().unwrap(),
            "--prior-mean",
            "0",
            "--prior-sd",
            "1",
            "--known-sigma",
            "1",
            "--particles",
            "1000",
            "--chains",
            "2",
            "--ess-target",
            "0.5",
            "--correlation-threshold",
            "0.01",
            "--seed",
            "10101",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "normal-mean-smc",
            "--input",
            input.to_str().unwrap(),
            "--prior-mean",
            "0",
            "--prior-sd",
            "1",
            "--known-sigma",
            "1",
            "--particles",
            "1000",
            "--chains",
            "2",
            "--ess-target",
            "0.5",
            "--correlation-threshold",
            "0.01",
            "--seed",
            "10101",
            "--timeout-seconds",
            "180",
            "--out",
            repeated_output.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read(&output).unwrap(),
        fs::read(repeated_output).unwrap()
    );

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("SMC JSON");
    assert_eq!(result["format"], "marklab.bayesian_normal_mean_smc");
    assert_eq!(result["fit_state"], "complete");
    assert!((result["posterior"]["mean"].as_f64().unwrap() - 2.0).abs() <= 0.1);
    assert!((result["posterior"]["sd"].as_f64().unwrap() - 0.2_f64.sqrt()).abs() <= 0.08);
    assert!((result["evidence"]["mean"].as_f64().unwrap() + 9.48047308903574).abs() <= 0.35);
    let chains = result["chains"].as_array().unwrap();
    assert_eq!(chains.len(), 2);
    for chain in chains {
        let stages = chain["stages"].as_array().unwrap();
        assert!(!stages.is_empty());
        assert_eq!(stages.last().unwrap()["beta"], 1.0);
        let mut prior_beta = 0.0;
        for stage in stages {
            let beta = stage["beta"].as_f64().unwrap();
            assert!(beta > prior_beta && beta <= 1.0);
            prior_beta = beta;
            assert!(stage["ess"].as_f64().unwrap() > 0.0);
            let ancestors = stage["ancestor_indexes"].as_array().unwrap();
            assert_eq!(ancestors.len(), 1000);
            assert!(ancestors.iter().all(|index| index.as_u64().unwrap() < 1000));
        }
    }
    assert_eq!(result["claim_status"], "experimental");
}

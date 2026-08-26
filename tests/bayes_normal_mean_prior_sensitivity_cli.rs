#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn conjugate_prior_grid_reports_posterior_predictive_and_decision_changes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let observations = directory.path().join("observations.csv");
    let priors = directory.path().join("priors.csv");
    let output = directory.path().join("sensitivity.json");
    fs::write(&observations, "observation\n1\n2\n3\n4\n").expect("observations");
    fs::write(
        &priors,
        "prior_name,prior_mean,prior_sd\nbase,0,1\nskeptical,-2,0.5\nwide,0,10\n",
    )
    .expect("priors");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "normal-mean-prior-sensitivity",
            "--input",
            observations.to_str().unwrap(),
            "--priors",
            priors.to_str().unwrap(),
            "--base-prior",
            "base",
            "--known-sigma",
            "1",
            "--decision-threshold",
            "1",
            "--decision-probability-threshold",
            "0.95",
            "--material-mean-shift",
            "0.5",
            "--timeout-seconds",
            "60",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("sensitivity JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_normal_mean_prior_sensitivity"
    );
    assert_eq!(result["base_prior"], "base");
    assert_eq!(result["priors"].as_array().unwrap().len(), 3);
    let base = result["priors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|prior| prior["prior_name"] == "base")
        .unwrap();
    assert!((base["posterior_mean"].as_f64().unwrap() - 2.0).abs() <= 1e-12);
    assert!((base["posterior_sd"].as_f64().unwrap() - 0.2_f64.sqrt()).abs() <= 1e-12);
    assert_eq!(base["decision"], true);
    assert!(base["loo_elpd"].as_f64().unwrap().is_finite());
    assert_eq!(
        result["conclusion_changed_priors"],
        serde_json::json!(["skeptical"])
    );
    assert_eq!(
        result["material_mean_shift_priors"],
        serde_json::json!(["skeptical"])
    );
    assert_eq!(result["sensitivity_state"], "decision_sensitive");
    assert_eq!(result["claim_status"], "experimental_sensitivity_analysis");
}

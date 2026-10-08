#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn normal_mean_nuts_matches_conjugate_oracle_and_reports_diagnostics() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(&input, "observation\n1\n2\n3\n4\n").expect("fixture");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    for output in [&first, &second] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "bayes",
                "normal-mean",
                "--input",
                input.to_str().expect("input path"),
                "--prior-mean",
                "0",
                "--prior-sd",
                "1",
                "--known-sigma",
                "1",
                "--chains",
                "2",
                "--tune",
                "500",
                "--draws",
                "1000",
                "--target-accept",
                "0.9",
                "--seed",
                "20260824",
                "--timeout-seconds",
                "180",
                "--out",
                output.to_str().expect("output path"),
            ])
            .assert()
            .success();
    }

    let first_bytes = fs::read(first).expect("first result");
    assert_eq!(first_bytes, fs::read(second).expect("second result"));
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_fit");
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["claim_status"], "experimental");
    assert_eq!(result["backend"]["name"], "pymc");
    assert_eq!(result["backend"]["version"], "6.3.0");
    assert_eq!(result["backend"]["python_version"], "3.12");
    assert_eq!(
        result["backend"]["worker_sha256"]
            .as_str()
            .expect("worker digest")
            .len(),
        64
    );
    assert_eq!(result["model"]["format"], "marklab.bayesian_model_ir");
    assert_eq!(result["model"]["version"], 1);
    assert_eq!(result["model"]["family"], "normal_mean_known_sigma");
    assert_eq!(result["model"]["parameter"]["name"], "mu");
    assert_eq!(result["model"]["observation_unit"], "scalar_observation");
    assert_eq!(result["sampling"]["chains"], 2);
    assert_eq!(result["sampling"]["draws_per_chain"], 1000);
    assert_eq!(result["sampling"]["completed_draws"], 2000);

    let posterior_mean = result["posterior"]["mean"].as_f64().expect("mean");
    let posterior_sd = result["posterior"]["sd"].as_f64().expect("sd");
    assert!((posterior_mean - 2.0).abs() <= 0.15, "{posterior_mean}");
    assert!(
        (posterior_sd - 0.2_f64.sqrt()).abs() <= 0.10,
        "{posterior_sd}"
    );

    let diagnostics = &result["diagnostics"];
    assert_eq!(diagnostics["prior_predictive_finite"], true);
    assert_eq!(diagnostics["posterior_finite"], true);
    assert_eq!(diagnostics["divergences"], 0);
    assert_eq!(diagnostics["max_tree_depth_hits"], 0);
    assert!(diagnostics["r_hat"].as_f64().expect("R-hat") <= 1.01);
    assert!(diagnostics["ess_bulk"].as_f64().expect("bulk ESS") >= 400.0);
    assert!(diagnostics["ess_tail"].as_f64().expect("tail ESS") >= 400.0);

    assert_eq!(result["posterior_predictive"]["observed_mean"], 2.5);
    assert!(result["posterior_predictive"]["replicated_mean_mean"]
        .as_f64()
        .expect("posterior-predictive mean")
        .is_finite());
}

#[test]
fn normal_mean_nuts_publishes_nonconverged_state_when_ess_policy_cannot_pass() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(&input, "observation\n1\n2\n3\n4\n").expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "normal-mean",
            "--input",
            input.to_str().expect("input path"),
            "--prior-mean",
            "0",
            "--prior-sd",
            "1",
            "--known-sigma",
            "1",
            "--chains",
            "2",
            "--tune",
            "100",
            "--draws",
            "100",
            "--target-accept",
            "0.9",
            "--seed",
            "7",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["fit_state"], "nonconverged");
    assert_eq!(result["claim_status"], "diagnostic_only_nonconverged");
    assert!(
        result["diagnostics"]["ess_bulk"]
            .as_f64()
            .expect("bulk ESS")
            < 400.0
    );
}

#![cfg(feature = "cli")]

use std::{f64::consts::PI, fmt::Write as _, fs};

use assert_cmd::Command;
use statrs::distribution::{ContinuousCDF, Normal};

const DATA_IDENTITY: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const PREPROCESSING_IDENTITY: &str =
    "2222222222222222222222222222222222222222222222222222222222222222";

#[test]
fn psis_loo_matches_conjugate_normal_leave_one_out_predictive() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("log-likelihood.csv");
    let output = directory.path().join("loo.json");
    let observations = [("p1", 1.0), ("p2", 2.0), ("p3", 3.0), ("p4", 4.0)];
    let posterior = Normal::new(2.0, 0.2_f64.sqrt()).expect("posterior");
    let mut csv = String::from("chain,draw,unit_id,log_likelihood\n");
    for chain in 0..2 {
        for draw in 0..1_000 {
            let probability = (chain * 1_000 + draw + 1) as f64 / 2_001.0;
            let mean_draw = posterior.inverse_cdf(probability);
            for (unit_id, observation) in observations {
                let log_likelihood =
                    -0.5 * (2.0 * PI).ln() - 0.5 * (observation - mean_draw).powi(2);
                writeln!(csv, "{chain},{draw},{unit_id},{log_likelihood:.17}").expect("CSV row");
            }
        }
    }
    fs::write(&input, csv).expect("input");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "psis-loo",
            "--input",
            input.to_str().unwrap(),
            "--model-name",
            "normal_model",
            "--likelihood-target",
            "normal_observation",
            "--data-identity-sha256",
            DATA_IDENTITY,
            "--preprocessing-identity-sha256",
            PREPROCESSING_IDENTITY,
            "--heldout-unit",
            "patient",
            "--relative-efficiency",
            "1",
            "--timeout-seconds",
            "60",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("PSIS-LOO JSON");
    assert_eq!(result["format"], "marklab.bayesian_psis_loo");
    assert_eq!(result["heldout_unit"], "patient");
    assert_eq!(result["chains"], 2);
    assert_eq!(result["draws_per_chain"], 1_000);
    assert_eq!(result["sample_count"], 2_000);
    assert_eq!(result["pointwise"].as_array().unwrap().len(), 4);

    let exact_elpd = observations
        .iter()
        .map(|(_, observation)| {
            let other_sum = observations.iter().map(|(_, value)| value).sum::<f64>() - observation;
            let predictive_mean = other_sum / 4.0;
            let predictive_variance = 1.25;
            -0.5 * (2.0 * PI * predictive_variance).ln()
                - 0.5 * (observation - predictive_mean).powi(2) / predictive_variance
        })
        .sum::<f64>();
    assert!((result["elpd_loo"].as_f64().unwrap() - exact_elpd).abs() <= 0.03);
    assert!(result["p_loo"].as_f64().unwrap() > 0.0);
    assert_eq!(result["reliability"], "reliable");
    assert!(result["refit_or_kfold_units"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(
        result["maximum_pareto_k"].as_f64().unwrap() <= result["good_pareto_k"].as_f64().unwrap()
    );
    assert_eq!(result["claim_status"], "experimental_predictive_diagnostic");
}

#[test]
fn psis_loo_retains_high_pareto_k_refit_warning() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("influential-log-likelihood.csv");
    let output = directory.path().join("influential-loo.json");
    let mut csv = String::from("chain,draw,unit_id,log_likelihood\n");
    for chain in 0..2 {
        for draw in 0..1_000 {
            let probability = (chain * 1_000 + draw) as f64 + 0.5;
            let log_tail = (1.0 - probability / 2_000.0).ln();
            writeln!(csv, "{chain},{draw},good,{:.17}", 0.2 * log_tail).expect("good row");
            writeln!(csv, "{chain},{draw},bad,{:.17}", 1.2 * log_tail).expect("bad row");
        }
    }
    fs::write(&input, csv).expect("input");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "psis-loo",
            "--input",
            input.to_str().unwrap(),
            "--model-name",
            "influential_model",
            "--likelihood-target",
            "synthetic_observation",
            "--data-identity-sha256",
            DATA_IDENTITY,
            "--preprocessing-identity-sha256",
            PREPROCESSING_IDENTITY,
            "--heldout-unit",
            "patient",
            "--relative-efficiency",
            "1",
            "--timeout-seconds",
            "60",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("PSIS-LOO JSON");
    assert_eq!(result["warning"], true);
    assert_eq!(result["reliability"], "requires_refit_or_kfold");
    assert_eq!(result["refit_or_kfold_units"], serde_json::json!(["bad"]));
    assert_eq!(result["pointwise"][0]["unit_id"], "bad");
    assert_eq!(
        result["pointwise"][0]["reliability"],
        "requires_refit_or_kfold"
    );
    assert!(
        result["pointwise"][0]["pareto_k"].as_f64().unwrap()
            > result["good_pareto_k"].as_f64().unwrap()
    );
}

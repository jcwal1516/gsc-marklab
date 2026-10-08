#![cfg(feature = "cli")]

use std::{f64::consts::PI, fmt::Write as _, fs, path::Path};

use assert_cmd::Command;
use statrs::distribution::{ContinuousCDF, Normal};

const DATA_IDENTITY: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const PREPROCESSING_IDENTITY: &str =
    "2222222222222222222222222222222222222222222222222222222222222222";

#[test]
fn compatible_psis_models_report_pairwise_elpd_difference() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input_a = directory.path().join("model-a.csv");
    let input_b = directory.path().join("model-b.csv");
    write_log_likelihood(&input_a, 0.0);
    write_log_likelihood(&input_b, -1.5);
    let loo_a = directory.path().join("model-a-loo.json");
    let loo_b = directory.path().join("model-b-loo.json");
    run_loo(&input_a, &loo_a, "model_a");
    run_loo(&input_b, &loo_b, "model_b");
    let output = directory.path().join("comparison.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "compare-models",
            "--input",
            loo_b.to_str().unwrap(),
            "--input",
            loo_a.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("comparison JSON");
    assert_eq!(result["format"], "marklab.bayesian_model_comparison");
    assert_eq!(result["heldout_unit"], "patient");
    assert_eq!(result["likelihood_target"], "normal_observation");
    assert_eq!(result["data_identity_sha256"], DATA_IDENTITY);
    assert_eq!(
        result["preprocessing_identity_sha256"],
        PREPROCESSING_IDENTITY
    );
    assert_eq!(result["reliability"], "reliable");
    assert_eq!(result["best_predictive_model"], "model_a");
    assert_eq!(result["models"][0]["model_name"], "model_a");
    assert_eq!(result["models"][0]["rank"], 1);
    assert_eq!(result["models"][1]["model_name"], "model_b");
    assert_eq!(result["models"][1]["rank"], 2);
    assert_eq!(result["pairwise"].as_array().unwrap().len(), 1);
    assert_eq!(result["pairwise"][0]["model_a"], "model_a");
    assert_eq!(result["pairwise"][0]["model_b"], "model_b");
    assert!((result["pairwise"][0]["elpd_difference"].as_f64().unwrap() - 6.0).abs() <= 1e-10);
    assert!(result["pairwise"][0]["standard_error"].as_f64().unwrap() <= 1e-10);
    assert_eq!(result["claim_status"], "experimental_predictive_comparison");

    let mut incompatible: serde_json::Value =
        serde_json::from_slice(&fs::read(&loo_b).unwrap()).expect("model B JSON");
    incompatible["preprocessing_identity_sha256"] = serde_json::Value::String("3".repeat(64));
    fs::write(&loo_b, serde_json::to_vec(&incompatible).unwrap()).expect("changed identity");
    let incompatible_output = directory.path().join("incompatible.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "compare-models",
            "--input",
            loo_a.to_str().unwrap(),
            "--input",
            loo_b.to_str().unwrap(),
            "--out",
            incompatible_output.to_str().unwrap(),
        ])
        .assert()
        .failure();
}

fn write_log_likelihood(path: &Path, shift: f64) {
    let observations = [("p1", 1.0), ("p2", 2.0), ("p3", 3.0), ("p4", 4.0)];
    let posterior = Normal::new(2.0, 0.2_f64.sqrt()).expect("posterior");
    let mut csv = String::from("chain,draw,unit_id,log_likelihood\n");
    for chain in 0..2 {
        for draw in 0..500 {
            let probability = (chain * 500 + draw + 1) as f64 / 1_001.0;
            let mean_draw = posterior.inverse_cdf(probability);
            for (unit_id, observation) in observations {
                let log_likelihood =
                    -0.5 * (2.0 * PI).ln() - 0.5 * (observation - mean_draw).powi(2) + shift;
                writeln!(csv, "{chain},{draw},{unit_id},{log_likelihood:.17}").expect("CSV row");
            }
        }
    }
    fs::write(path, csv).expect("input");
}

fn run_loo(input: &Path, output: &Path, model_name: &str) {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "psis-loo",
            "--input",
            input.to_str().unwrap(),
            "--model-name",
            model_name,
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
}

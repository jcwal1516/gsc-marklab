#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn synthetic_likelihood_mcmc_recovers_growth_rate_and_replays() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("initial.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "initial": [
                {"position_um": 0.0, "density": 0.25},
                {"position_um": 1.0, "density": 0.25},
                {"position_um": 2.0, "density": 0.25}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    run(&input, &first);
    run(&input, &second);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.synthetic_likelihood_growth_front"
    );
    assert!((result["posterior"]["growth_rate_mean"].as_f64().unwrap() - 1.0).abs() < 0.05);
    assert_eq!(result["likelihood"]["summary_dimension"], 2);
    assert_eq!(result["likelihood"]["replicates_per_evaluation"], 32);
    assert!(result["mcmc"]["acceptance_rate"].as_f64().unwrap() > 0.05);
    assert!(result["mcmc"]["acceptance_rate"].as_f64().unwrap() < 0.95);
    assert_eq!(result["mcmc"]["retained_draws"], 1000);
    assert_eq!(
        result["claim_status"],
        "approximate_synthetic_likelihood_not_biological_calibration"
    );
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "synthetic-likelihood-growth-front",
            "--input",
            input.to_str().unwrap(),
            "--diffusion-um2-per-time",
            "0",
            "--carrying-capacity",
            "1",
            "--final-time",
            "1.0986122886681098",
            "--time-step",
            "0.1",
            "--front-threshold-fraction",
            "0.5",
            "--observed-final-mass",
            "1",
            "--observed-maximum-density",
            "0.5",
            "--mass-noise-sd",
            "0.02",
            "--maximum-density-noise-sd",
            "0.01",
            "--growth-rate-prior-min",
            "0.5",
            "--growth-rate-prior-max",
            "1.5",
            "--replicates",
            "32",
            "--covariance-shrinkage",
            "0.5",
            "--iterations",
            "1500",
            "--burn-in",
            "500",
            "--proposal-sd",
            "0.04",
            "--maximum-cell-steps-per-simulation",
            "1000",
            "--seed",
            "20260825",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn posterior_predictive_lab_retains_well_calibrated_growth_control() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "initial": [
                {"position_um": 0.0, "density": 0.25},
                {"position_um": 1.0, "density": 0.25},
                {"position_um": 2.0, "density": 0.25}
            ],
            "posterior_growth_rate_draws": [0.95, 1.0, 1.05]
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
        "marklab.growth_front_posterior_predictive_laboratory"
    );
    assert_eq!(result["replicates"].as_array().unwrap().len(), 100);
    assert_eq!(result["checks"].as_array().unwrap().len(), 2);
    assert!(result["checks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|check| check["observed_in_interval"] == true));
    assert!(result["misspecification_flags"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(result["failed_replicates"], 0);
    assert_eq!(
        result["random_seed_namespace"],
        "growth_front_ppc_lab_v1_chacha20"
    );
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "growth-front-posterior-predictive-lab",
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
            "--replicates",
            "100",
            "--interval-probability",
            "0.9",
            "--discrepancy-alpha",
            "0.05",
            "--maximum-cell-steps-per-replicate",
            "1000",
            "--seed",
            "20260825",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

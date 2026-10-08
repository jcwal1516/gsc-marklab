#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn fixed_step_hmc_recovers_conjugate_normal_posterior_and_retains_energy_diagnostics() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("hmc.json");
    let fixture = serde_json::json!({
        "observations":[1.8,2.1,1.9,2.2,2.0,1.7,2.3,2.0],
        "observation_standard_deviation":0.5,
        "prior_mean":0.0,"prior_standard_deviation":2.0,
        "initial_state":0.0,"mass":1.0,"step_size":0.05,"leapfrog_steps":20,
        "warmup":500,"draws":2000,"seed":307,"timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "hmc-normal",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.fixed_step_hmc_normal_mean");
    let analytic = result["analytic_posterior"]["mean"].as_f64().unwrap();
    assert!((result["posterior"]["mean"].as_f64().unwrap() - analytic).abs() < 0.04);
    assert!(result["diagnostics"]["acceptance_rate"].as_f64().unwrap() > 0.7);
    assert!(
        result["diagnostics"]["maximum_absolute_energy_error"]
            .as_f64()
            .unwrap()
            < 0.2
    );
    assert_eq!(result["diagnostics"]["constraint_transform_jacobian"], 0.0);
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_fixed_step_hmc"
    );
}

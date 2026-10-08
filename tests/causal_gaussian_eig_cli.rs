#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn nested_monte_carlo_eig_matches_gaussian_oracle_and_replays() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "candidate_id": "unit_sensitivity",
            "prior_mean": 0.0,
            "prior_sd": 1.0,
            "sensitivity": 1.0,
            "noise_sd": 1.0,
            "outer_samples": 1000,
            "inner_samples": 500,
            "seed": 20260825,
            "maximum_likelihood_evaluations": 600000
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
        "marklab.gaussian_expected_information_gain"
    );
    let analytic = result["analytic_eig"].as_f64().unwrap();
    assert!((analytic - 0.5 * 2.0_f64.ln()).abs() < 1.0e-15);
    let estimate = result["estimate"].as_f64().unwrap();
    let se = result["monte_carlo_se"].as_f64().unwrap();
    assert!((estimate - analytic).abs() <= 4.0 * se + 0.05);
    assert_eq!(result["likelihood_evaluations"], 501000);
    assert_eq!(
        result["claim_status"],
        "analytic_scalar_design_utility_only"
    );
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "gaussian-eig",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

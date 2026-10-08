#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn ekf_and_ukf_reduce_to_exact_linear_filter() {
    let directory = tempfile::tempdir().unwrap();
    for method in ["ekf", "ukf"] {
        let input = directory.path().join(format!("{method}.json"));
        fs::write(
            &input,
            serde_json::to_vec(&serde_json::json!({
                "method": method,
                "observations": [1.0, 2.0],
                "steps": [
                    linear_identity_step(),
                    linear_identity_step()
                ],
                "initial_mean": 0.0,
                "initial_variance": 1.0,
                "ukf_alpha": 0.5,
                "ukf_beta": 2.0,
                "ukf_kappa": 0.0,
                "maximum_time_steps": 10
            }))
            .unwrap(),
        )
        .unwrap();
        let first = directory.path().join(format!("{method}-first.json"));
        let second = directory.path().join(format!("{method}-second.json"));
        run(&input, &first);
        run(&input, &second);
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
        let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
        assert_eq!(result["format"], "marklab.scalar_nonlinear_gaussian_filter");
        assert_eq!(result["method"], method);
        assert_eq!(result["observed_updates"], 2);
        let states = result["filtered_states"].as_array().unwrap();
        assert_close(states[0]["mean"].as_f64().unwrap(), 0.5, 1.0e-10);
        assert_close(states[1]["mean"].as_f64().unwrap(), 1.0, 1.0e-10);
        assert_close(states[1]["variance"].as_f64().unwrap(), 1.0 / 3.0, 1.0e-10);
    }
}

fn linear_identity_step() -> serde_json::Value {
    serde_json::json!({
        "transition": {"intercept": 0.0, "linear": 1.0, "quadratic": 0.0},
        "process_variance": 0.0,
        "observation": {"intercept": 0.0, "linear": 1.0, "quadratic": 0.0},
        "observation_variance": 1.0
    })
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "longitudinal",
            "nonlinear-filter",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected}"
    );
}

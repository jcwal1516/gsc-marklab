#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn scalar_filter_and_smoother_match_hand_oracle_and_replay() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "observations": [[1.0], [2.0]],
            "transition_matrices": [[[1.0]], [[1.0]]],
            "process_covariances": [[[0.0]], [[0.0]]],
            "observation_matrices": [[[1.0]], [[1.0]]],
            "observation_covariances": [[[1.0]], [[1.0]]],
            "initial_mean": [0.0],
            "initial_covariance": [[1.0]],
            "maximum_time_steps": 10,
            "maximum_state_dimension": 4,
            "maximum_observation_dimension": 4,
            "maximum_matrix_operations": 10000
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
    assert_eq!(result["format"], "marklab.linear_gaussian_state_space");
    assert_eq!(result["version"], 1);
    assert_eq!(result["algorithm"], "kalman_joseph_rts_cholesky");
    assert_eq!(result["observed_updates"], 2);

    let filtered = result["filtered_states"].as_array().unwrap();
    assert_close(filtered[0]["mean"][0].as_f64().unwrap(), 0.5, 1.0e-10);
    assert_close(filtered[1]["mean"][0].as_f64().unwrap(), 1.0, 1.0e-10);
    assert_close(
        filtered[1]["covariance"][0][0].as_f64().unwrap(),
        1.0 / 3.0,
        1.0e-10,
    );
    let smoothed = result["smoothed_states"].as_array().unwrap();
    assert_close(smoothed[0]["mean"][0].as_f64().unwrap(), 1.0, 1.0e-9);
    assert_close(smoothed[1]["mean"][0].as_f64().unwrap(), 1.0, 1.0e-10);
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "longitudinal",
            "kalman-smooth",
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

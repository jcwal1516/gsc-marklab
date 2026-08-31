#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "longitudinal-kalman-smooth",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn kalman_rts_replays_across_processes_with_the_hand_oracle_intact() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &input,
        br#"{"observations":[[1.0],[2.0]],"transition_matrices":[[[1.0]],[[1.0]]],"process_covariances":[[[0.0]],[[0.0]]],"observation_matrices":[[[1.0]],[[1.0]]],"observation_covariances":[[[1.0]],[[1.0]]],"initial_mean":[0.0],"initial_covariance":[[1.0]],"maximum_time_steps":10,"maximum_state_dimension":4,"maximum_observation_dimension":4,"maximum_matrix_operations":10000}"#,
    )
    .expect("fixture");

    project_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.linear_gaussian_state_space");
    for (actual, expected) in [
        (
            result["filtered_states"][0]["mean"][0].as_f64().unwrap(),
            0.5,
        ),
        (
            result["filtered_states"][1]["mean"][0].as_f64().unwrap(),
            1.0,
        ),
        (
            result["smoothed_states"][0]["mean"][0].as_f64().unwrap(),
            1.0,
        ),
    ] {
        assert!(
            (actual - expected).abs() <= 1.0e-9,
            "{actual} != {expected}"
        );
    }
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

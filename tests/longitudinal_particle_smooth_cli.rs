#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn deterministic_particles_and_ancestry_smoothing_are_exact_and_replay() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "observations": [null, null, null],
            "steps": [step(), step(), step()],
            "initial_mean": 0.0,
            "initial_sd": 0.0,
            "particles": 16,
            "ess_resampling_fraction": 0.5,
            "smoothed_trajectories": 4,
            "seed": 20260825,
            "maximum_particle_steps": 1000
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
        "marklab.scalar_bootstrap_particle_smoother"
    );
    assert_eq!(result["particle_steps"], 48);
    assert_eq!(result["marginal_log_likelihood"], 0.0);
    let steps = result["filtering_steps"].as_array().unwrap();
    for (index, expected) in [1.0, 2.0, 3.0].into_iter().enumerate() {
        assert_eq!(steps[index]["weighted_mean"].as_f64().unwrap(), expected);
        assert_eq!(steps[index]["resampled"], false);
    }
    let trajectories = result["smoothed_trajectories"].as_array().unwrap();
    assert_eq!(trajectories.len(), 4);
    for trajectory in trajectories {
        assert_eq!(trajectory["states"], serde_json::json!([1.0, 2.0, 3.0]));
    }
}

fn step() -> serde_json::Value {
    serde_json::json!({
        "transition": {"intercept": 1.0, "linear": 1.0, "quadratic": 0.0},
        "process_sd": 0.0,
        "observation": {"intercept": 0.0, "linear": 1.0, "quadratic": 0.0},
        "observation_sd": 1.0
    })
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "longitudinal",
            "particle-smooth",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

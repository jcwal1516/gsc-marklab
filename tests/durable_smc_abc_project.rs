#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "smc-abc-growth-front",
        "--project",
        project.to_str().unwrap(),
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
        "--mass-scale",
        "0.02",
        "--growth-rate-prior-min",
        "0.5",
        "--growth-rate-prior-max",
        "1.5",
        "--epsilon-schedule",
        "5,2,1",
        "--particles",
        "64",
        "--maximum-proposals-per-stage",
        "10000",
        "--maximum-cell-steps-per-proposal",
        "1000",
        "--seed",
        "20260825",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn smc_abc_replays_across_processes_without_repeating_simulations() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("initial.json");
    let project = directory.path().join("project");
    let direct = directory.path().join("direct.json");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &input,
        br#"{"initial":[{"position_um":0.0,"density":0.25},{"position_um":1.0,"density":0.25},{"position_um":2.0,"density":0.25}]}"#,
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "smc-abc-growth-front",
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
            "--mass-scale",
            "0.02",
            "--growth-rate-prior-min",
            "0.5",
            "--growth-rate-prior-max",
            "1.5",
            "--epsilon-schedule",
            "5,2,1",
            "--particles",
            "64",
            "--maximum-proposals-per-stage",
            "10000",
            "--maximum-cell-steps-per-proposal",
            "1000",
            "--seed",
            "20260825",
            "--out",
            direct.to_str().unwrap(),
        ])
        .assert()
        .success();

    project_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let direct_result: serde_json::Value =
        serde_json::from_slice(&fs::read(&direct).unwrap()).unwrap();
    let project_result: serde_json::Value =
        serde_json::from_slice(&fs::read(&first).unwrap()).unwrap();
    assert_eq!(direct_result, project_result);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

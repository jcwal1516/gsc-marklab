#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "spatial-varying-coefficient",
        "--project",
        project.to_str().unwrap(),
    ]);
    analysis_args(&mut command, input, output);
    command
}

fn analysis_args(command: &mut Command, input: &Path, output: &Path) {
    command.args([
        "--input",
        input.to_str().unwrap(),
        "--global-predictor-name",
        "global_x",
        "--spatial-predictor-name",
        "spatial_z",
        "--intercept-prior-mean",
        "0",
        "--intercept-prior-sd",
        "2",
        "--coefficient-prior-sd",
        "2",
        "--amplitude-prior-sd",
        "1",
        "--length-scale-prior-sd-um",
        "5",
        "--known-noise-sd",
        "0.08",
        "--jitter",
        "0.000001",
        "--chains",
        "2",
        "--tune",
        "200",
        "--draws",
        "100",
        "--target-accept",
        "0.9",
        "--seed",
        "9101",
        "--timeout-seconds",
        "180",
        "--out",
        output.to_str().unwrap(),
    ]);
}

#[test]
fn pinned_spatial_varying_coefficient_replays_without_a_second_backend_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.csv");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &input,
        "coordinate_id,x_um,y,global_x,spatial_z\n\
p00,0,0.62,-1,0.5\n\
p01,1,1.5567499630944013,1,0.6\n\
p02,2,0.9778792717770607,-1,0.7\n\
p03,3,1.8821858797087616,1,0.8\n\
p04,4,1.1894442079008758,-1,0.9\n\
p05,5,1.9252234140045912,1,1.0\n\
p06,6,1.0570929708088244,-1,1.1\n\
p07,7,1.7987217360155938,1,1.2\n",
    )
    .expect("input");

    command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_spatial_varying_coefficient_fit"
    );
    assert_eq!(result["model"]["kernel"], "matern_3_2");
    assert_eq!(result["seed"], 9101);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

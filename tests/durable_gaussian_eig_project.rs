#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "gaussian-eig",
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
fn gaussian_design_utility_replays_with_the_analytic_oracle_intact() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &input,
        br#"{"candidate_id":"unit_sensitivity","prior_mean":0.0,"prior_sd":1.0,"sensitivity":1.0,"noise_sd":1.0,"outer_samples":200,"inner_samples":100,"seed":20260825,"maximum_likelihood_evaluations":30000}"#,
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
    let analytic = result["analytic_eig"].as_f64().unwrap();
    assert!((analytic - 0.5 * 2.0_f64.ln()).abs() <= 1.0e-15);
    assert_eq!(result["likelihood_evaluations"], 20200);
    assert_eq!(
        result["claim_status"],
        "analytic_scalar_design_utility_only"
    );
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

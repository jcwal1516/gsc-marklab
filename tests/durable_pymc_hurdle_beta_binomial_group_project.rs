#![cfg(feature = "cli")]

use std::{fmt::Write, fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "hurdle-beta-binomial-group",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "MSS",
        "--comparison-group",
        "MSI",
        "--presence-intercept-prior-mean",
        "0",
        "--presence-intercept-prior-sd",
        "2",
        "--presence-group-effect-prior-sd",
        "2",
        "--abundance-intercept-prior-mean",
        "-2",
        "--abundance-intercept-prior-sd",
        "2",
        "--abundance-group-effect-prior-sd",
        "2",
        "--concentration-prior-sd",
        "50",
        "--chains",
        "4",
        "--tune",
        "600",
        "--draws",
        "600",
        "--target-accept",
        "0.95",
        "--seed",
        seed,
        "--timeout-seconds",
        "180",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn hurdle_beta_binomial_group_runs_once_then_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_counts.csv");
    let mut csv = String::from("patient_id,group,successes,trials\n");
    for index in 0..12 {
        let successes = if index < 8 { 0 } else { 2 + index % 3 };
        writeln!(csv, "mss-{index},MSS,{successes},100").expect("reference row");
    }
    for index in 0..12 {
        let successes = if index < 2 { 0 } else { 25 + index % 6 };
        writeln!(csv, "msi-{index},MSI,{successes},100").expect("comparison row");
    }
    fs::write(&input, csv).expect("fixture");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    command(&project, &input, &first, "20260831")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second, "20260831")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_hurdle_beta_binomial_group"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");

    command(
        &project,
        &input,
        &directory.path().join("changed.json"),
        "20260901",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));
    let records = fs::read_to_string(project.join("executions.jsonl")).unwrap();
    assert_eq!(records.lines().count(), 1);
    let record: serde_json::Value = serde_json::from_str(records.lines().next().unwrap()).unwrap();
    assert_eq!(
        record["identity"]["node"]["id"],
        "pymc-hurdle-beta-binomial-group"
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_hurdle_beta_binomial_group_worker_result"
    );
}

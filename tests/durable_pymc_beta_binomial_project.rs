#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "beta-binomial-hierarchy",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--population-alpha",
        "2",
        "--population-beta",
        "2",
        "--concentration-prior-sd",
        "20",
        "--chains",
        "2",
        "--tune",
        "1500",
        "--draws",
        "2000",
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
fn beta_binomial_hierarchy_runs_once_then_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    fs::write(
        &input,
        "patient_id,successes,trials\n\
p-1,12,100\n\
p-2,18,100\n\
p-3,22,100\n\
p-4,27,100\n\
p-5,31,100\n\
p-6,35,100\n\
p-7,40,100\n\
p-8,45,100\n",
    )
    .expect("fixture");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    command(&project, &input, &first, "20260827")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second, "20260827")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.bayesian_beta_binomial_hierarchy");
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["backend"]["name"], "pymc");
    assert_eq!(result["model"]["family"], "patient_beta_binomial_hierarchy");

    command(
        &project,
        &input,
        &directory.path().join("changed.json"),
        "20260828",
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
        "pymc-beta-binomial-hierarchy"
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_beta_binomial_hierarchy_worker_result"
    );
}

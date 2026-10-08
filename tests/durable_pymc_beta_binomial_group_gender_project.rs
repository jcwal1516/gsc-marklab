#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "beta-binomial-group-gender-regression",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "MSS",
        "--comparison-group",
        "MSI",
        "--reference-gender",
        "Male",
        "--comparison-gender",
        "Female",
        "--intercept-prior-mean",
        "0",
        "--intercept-prior-sd",
        "2",
        "--group-effect-prior-sd",
        "1",
        "--gender-effect-prior-sd",
        "1",
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
fn group_gender_regression_runs_once_then_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    fs::write(
        &input,
        "patient_id,group,gender,successes,trials\n\
mss-m-1,MSS,Male,18,100\n\
mss-m-2,MSS,Male,20,100\n\
mss-m-3,MSS,Male,22,100\n\
mss-m-4,MSS,Male,24,100\n\
mss-f-1,MSS,Female,28,100\n\
mss-f-2,MSS,Female,30,100\n\
mss-f-3,MSS,Female,32,100\n\
mss-f-4,MSS,Female,34,100\n\
msi-m-1,MSI,Male,48,100\n\
msi-m-2,MSI,Male,50,100\n\
msi-m-3,MSI,Male,52,100\n\
msi-m-4,MSI,Male,54,100\n\
msi-f-1,MSI,Female,58,100\n\
msi-f-2,MSI,Female,60,100\n\
msi-f-3,MSI,Female,62,100\n\
msi-f-4,MSI,Female,64,100\n",
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
    assert_eq!(
        result["format"],
        "marklab.bayesian_beta_binomial_group_gender_regression"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");

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
        "pymc-beta-binomial-group-gender-regression"
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_beta_binomial_group_gender_regression_worker_result"
    );
}

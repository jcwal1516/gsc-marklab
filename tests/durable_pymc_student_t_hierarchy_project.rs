#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "student-t-hierarchy",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--global-prior-mean",
        "2.5",
        "--global-prior-sd",
        "3",
        "--between-patient-sd-prior",
        "2",
        "--observation-sd-prior",
        "1",
        "--degrees-of-freedom-excess-rate",
        "0.1",
        "--chains",
        "2",
        "--tune",
        "1500",
        "--draws",
        "2000",
        "--target-accept",
        "0.99",
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
fn student_t_hierarchy_runs_once_then_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(&input, "patient_id,observation\np-1,0\np-1,0.1\np-1,0.2\np-1,8\np-2,1\np-2,1.1\np-2,1.2\np-2,1.3\np-3,2\np-3,2.1\np-3,2.2\np-3,2.3\np-4,3\np-4,3.1\np-4,3.2\np-4,3.3\np-5,4\np-5,4.1\np-5,4.2\np-5,4.3\np-6,5\np-6,5.1\np-6,5.2\np-6,5.3\n").expect("fixture");
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
    assert_eq!(result["format"], "marklab.bayesian_student_t_hierarchy");
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["backend"]["name"], "pymc");
    assert_eq!(
        result["model"]["family"],
        "student_t_patient_varying_intercept"
    );

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
    assert_eq!(record["identity"]["node"]["id"], "pymc-student-t-hierarchy");
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_student_t_hierarchy_worker_result"
    );
}

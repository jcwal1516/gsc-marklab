#![cfg(feature = "cli")]

use std::{fmt::Write, fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "ordinal-group",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "MSS",
        "--comparison-group",
        "MSI",
        "--ordered-levels",
        "I,II,III,IV",
        "--cutpoint-prior-sd",
        "2",
        "--group-effect-prior-sd",
        "2",
        "--chains",
        "4",
        "--tune",
        "400",
        "--draws",
        "400",
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
fn ordinal_group_runs_once_then_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_stage.csv");
    let mut csv = String::from("patient_id,group,outcome\n");
    for index in 0..12 {
        writeln!(
            csv,
            "mss-{index},MSS,{}",
            if index < 9 { "I" } else { "II" }
        )
        .expect("reference row");
    }
    for index in 0..12 {
        writeln!(
            csv,
            "msi-{index},MSI,{}",
            if index < 3 { "III" } else { "IV" }
        )
        .expect("comparison row");
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
    assert_eq!(result["format"], "marklab.bayesian_ordinal_group");
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["input"]["patient_count"], 24);

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
    assert_eq!(record["identity"]["node"]["id"], "pymc-ordinal-group");
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_ordinal_group_worker_result"
    );
}

#![cfg(feature = "cli")]

use std::{fmt::Write, fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "dirichlet-multinomial-group",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "MSS",
        "--comparison-group",
        "MSI",
        "--logit-prior-sd",
        "1.5",
        "--group-effect-prior-sd",
        "1",
        "--concentration-prior-sd",
        "50",
        "--chains",
        "2",
        "--tune",
        "1200",
        "--draws",
        "1600",
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
fn dirichlet_multinomial_group_runs_once_then_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_classes.csv");
    let mut csv = String::from("patient_id,group,class_id,count\n");
    for index in 0..8 {
        for (class_id, count) in [
            ("Neoplastic", 68 + index),
            ("Inflammatory", 22 - index / 2),
            ("Connective", 10 - index / 2),
        ] {
            writeln!(csv, "mss-{index},MSS,{class_id},{count}").expect("row");
        }
    }
    for index in 0..8 {
        for (class_id, count) in [
            ("Neoplastic", 38 - index / 2),
            ("Inflammatory", 42 + index),
            ("Connective", 20 - index / 2),
        ] {
            writeln!(csv, "msi-{index},MSI,{class_id},{count}").expect("row");
        }
    }
    fs::write(&input, csv).expect("fixture");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    command(&project, &input, &first, "20260828")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second, "20260828")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_dirichlet_multinomial_group"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["input"]["patient_count"], 16);
    assert_eq!(result["input"]["class_count"], 3);

    command(
        &project,
        &input,
        &directory.path().join("changed.json"),
        "20260829",
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
        "pymc-dirichlet-multinomial-group"
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_dirichlet_multinomial_group_worker_result"
    );
}

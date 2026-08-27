#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::ContentDigest;

fn project_hierarchical_command(
    project: &Path,
    input: &Path,
    output: &Path,
    seed: &str,
) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "hierarchical-normal",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--global-prior-mean",
        "0",
        "--global-prior-sd",
        "5",
        "--between-patient-sd-prior",
        "2",
        "--known-sigma",
        "1",
        "--chains",
        "2",
        "--tune",
        "1000",
        "--draws",
        "2000",
        "--target-accept",
        "0.9",
        "--seed",
        seed,
        "--timeout-seconds",
        "180",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn hierarchical_normal_runs_once_then_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(
        &input,
        "patient_id,observation\n\
p-1,0.2\n\
p-1,0.4\n\
p-1,0.6\n\
p-1,0.8\n\
p-2,0.8\n\
p-2,1.0\n\
p-2,1.2\n\
p-2,1.4\n\
p-3,1.4\n\
p-3,1.6\n\
p-3,1.8\n\
p-3,2.0\n\
p-4,2.0\n\
p-4,2.2\n\
p-4,2.4\n\
p-4,2.6\n\
p-5,2.6\n\
p-5,2.8\n\
p-5,3.0\n\
p-5,3.2\n\
p-6,3.2\n\
p-6,3.4\n\
p-6,3.6\n\
p-6,3.8\n",
    )
    .expect("fixture");
    let input_digest = ContentDigest::from_bytes(&fs::read(&input).expect("input bytes"));
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    project_hierarchical_command(&project, &input, &first, "20260825")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_hierarchical_command(&project, &input, &second, "20260825")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let first_bytes = fs::read(&first).expect("first result");
    assert_eq!(first_bytes, fs::read(&second).expect("second result"));
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).expect("result JSON");
    assert_eq!(result["format"], "marklab.bayesian_hierarchical_fit");
    assert_eq!(result["fit_state"], "complete", "{}", result["diagnostics"]);
    assert_eq!(result["backend"]["name"], "pymc");
    assert_eq!(result["backend"]["version"], "6.3.0");
    assert_eq!(
        result["model"]["family"],
        "gaussian_patient_varying_intercept"
    );
    assert_eq!(result["input"]["patients"], 6);
    assert_eq!(result["input"]["observations"], 24);

    project_hierarchical_command(
        &project,
        &input,
        &directory.path().join("changed-seed.json"),
        "20260826",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));

    fs::write(
        &input,
        format!("{}\n", fs::read_to_string(&input).expect("input text")),
    )
    .expect("changed source bytes");
    project_hierarchical_command(
        &project,
        &input,
        &directory.path().join("changed-input.json"),
        "20260825",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(
        records.len(),
        1,
        "a replay and failed misses must not append"
    );
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    assert_eq!(record["identity"]["node"]["id"], "pymc-hierarchical-normal");
    assert_eq!(
        record["identity"]["inputs"][0]["digest"],
        input_digest.to_string()
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_hierarchical_worker_result"
    );
}

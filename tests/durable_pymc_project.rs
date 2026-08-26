#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::ContentDigest;

fn project_normal_mean_command(project: &Path, input: &Path, output: &Path) -> Command {
    project_normal_mean_command_with_seed(project, input, output, "20260824")
}

fn project_normal_mean_command_with_seed(
    project: &Path,
    input: &Path,
    output: &Path,
    seed: &str,
) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "normal-mean",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--prior-mean",
        "0",
        "--prior-sd",
        "1",
        "--known-sigma",
        "1",
        "--chains",
        "2",
        "--tune",
        "500",
        "--draws",
        "1000",
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
fn normal_mean_runs_once_then_replays_exact_typed_pymc_result_across_processes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(&input, "observation\n1\n2\n3\n4\n").expect("fixture");
    let input_digest = ContentDigest::from_bytes(&fs::read(&input).expect("input bytes"));
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    project_normal_mean_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_normal_mean_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let first_bytes = fs::read(&first).expect("first result");
    assert_eq!(first_bytes, fs::read(&second).expect("second result"));
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).expect("result JSON");
    assert_eq!(result["format"], "marklab.bayesian_fit");
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["backend"]["name"], "pymc");
    assert_eq!(result["backend"]["version"], "6.3.0");
    assert_eq!(result["backend"]["python_version"], "3.12");
    let posterior_mean = result["posterior"]["mean"].as_f64().expect("mean");
    assert!((posterior_mean - 2.0).abs() <= 0.15, "{posterior_mean}");

    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lock = fs::read(repository.join("workers/python/uv.lock")).expect("lock");
    let worker =
        fs::read(repository.join("workers/python/marklab_pymc_worker.py")).expect("worker");
    assert_eq!(
        result["backend"]["environment_lock_sha256"],
        marklab_bayes::sha256_hex(&lock)
    );
    assert_eq!(
        result["backend"]["worker_sha256"],
        marklab_bayes::sha256_hex(&worker)
    );

    project_normal_mean_command_with_seed(
        &project,
        &input,
        &directory.path().join("changed-seed.json"),
        "20260825",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));
    fs::write(&input, "observation\n1\n2\n3\n4\n\n").expect("changed source bytes");
    project_normal_mean_command(
        &project,
        &input,
        &directory.path().join("changed-input.json"),
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "a replay must not append an execution");
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    assert_eq!(record["identity"]["node"]["id"], "pymc-normal-mean");
    assert_eq!(
        record["identity"]["inputs"][0]["digest"],
        input_digest.to_string()
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_worker_result"
    );
}

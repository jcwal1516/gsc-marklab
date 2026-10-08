#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::ContentDigest;

fn project_fgw_command(project: &Path, input: &Path, output: &Path) -> Command {
    project_fgw_command_with_alpha(project, input, output, "0.5")
}

fn project_fgw_command_with_alpha(
    project: &Path,
    input: &Path,
    output: &Path,
    alpha: &str,
) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "fused-gromov-wasserstein",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--alpha",
        alpha,
        "--epsilon",
        "0.05",
        "--feature-scale",
        "1",
        "--structure-scale",
        "1",
        "--tolerance",
        "1e-9",
        "--maximum-iterations",
        "100",
        "--timeout-seconds",
        "60",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn fgw_runs_once_then_replays_exact_typed_pot_result_across_processes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "source": [
                {"id": "s1", "mass": 0.5, "features": [0.0]},
                {"id": "s2", "mass": 0.5, "features": [1.0]}
            ],
            "target": [
                {"id": "t1", "mass": 0.5, "features": [1.0]},
                {"id": "t2", "mass": 0.5, "features": [0.0]}
            ],
            "source_structure_row_major": [0.0, 1.0, 1.0, 0.0],
            "target_structure_row_major": [0.0, 1.0, 1.0, 0.0]
        }))
        .expect("input JSON"),
    )
    .expect("input");
    let input_digest = ContentDigest::from_bytes(&fs::read(&input).expect("input bytes"));
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    project_fgw_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_fgw_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let first_bytes = fs::read(&first).expect("first result");
    assert_eq!(first_bytes, fs::read(&second).expect("second result"));
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).expect("result JSON");
    assert_eq!(result["format"], "marklab.fused_gromov_wasserstein");
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["backend"]["name"], "pot");
    assert_eq!(result["backend"]["version"], "0.9.7.post1");
    assert_eq!(result["backend"]["python_version"], "3.12");
    let cross_mass = result["best_plan"]
        .as_array()
        .expect("plan")
        .iter()
        .filter(|row| {
            (row["source_id"] == "s1" && row["target_id"] == "t2")
                || (row["source_id"] == "s2" && row["target_id"] == "t1")
        })
        .map(|row| row["mass"].as_f64().expect("mass"))
        .sum::<f64>();
    assert!(cross_mass > 0.9, "cross mass was {cross_mass}");

    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lock = fs::read(repository.join("workers/python/uv.lock")).expect("lock");
    let worker = fs::read(repository.join("workers/python/marklab_pot_fused_gromov_worker.py"))
        .expect("worker");
    assert_eq!(
        result["backend"]["environment_lock_sha256"],
        marklab_bayes::sha256_hex(&lock)
    );
    assert_eq!(
        result["backend"]["worker_sha256"],
        marklab_bayes::sha256_hex(&worker)
    );

    project_fgw_command_with_alpha(
        &project,
        &input,
        &directory.path().join("changed-alpha.json"),
        "0.6",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));
    let mut changed_input = fs::read(&input).expect("input bytes");
    changed_input.push(b'\n');
    fs::write(&input, changed_input).expect("changed source bytes");
    project_fgw_command(
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
    assert_eq!(
        record["identity"]["node"]["id"],
        "pot-fused-gromov-wasserstein"
    );
    assert_eq!(
        record["identity"]["inputs"][0]["digest"],
        input_digest.to_string()
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pot_fused_gromov_wasserstein_worker_result"
    );
}

#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

fn fixture(path: &Path) {
    let mut csv = String::from(
        "patient_id,outer_fold,inner_fold,target,technical_0,clinical_0,compartment_0,acquisition_0,cell_0,patch_0,neighbor_0,measured_0\n",
    );
    for index in 0..24 {
        let target = index as f64 * 0.75 - 4.0;
        writeln!(
            csv,
            "p{index},{},{},{target},{},{},{},{},{},{target},{},{}",
            index % 4,
            (index / 4) % 3,
            index % 2,
            index % 3,
            index % 4,
            index % 5,
            (index as f64 * 0.37).sin(),
            (index as f64 * 0.23).cos(),
            (index as f64 * 0.19).sin(),
        )
        .expect("fixture row");
    }
    fs::write(path, csv).expect("fixture");
}

fn project_command(
    binary: &Path,
    project: &Path,
    input: &Path,
    output: &Path,
    permutations: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.env("MARKLAB_RUNTIME_ROOT", env!("CARGO_MANIFEST_DIR"));
    command.args([
        "project",
        "test-cell-patch-complementarity",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--outer-folds",
        "4",
        "--inner-folds",
        "3",
        "--ridge-alphas",
        "0,0.01,0.1,1,10",
        "--permutations",
        permutations,
        "--seed",
        "1409",
        "--timeout-seconds",
        "120",
        "--memory-budget-mib",
        "8",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn complementarity_is_admitted_then_replays_without_a_second_scipy_process() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable executable");
    let input = directory.path().join("patients.csv");
    fixture(&input);

    let invalid_project = directory.path().join("invalid-project");
    let invalid_output = directory.path().join("invalid.json");
    project_command(&binary, &invalid_project, &input, &invalid_output, "0")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "complementarity permutations exceed the paired-patient resource bound",
        ));
    assert!(
        !invalid_project.exists(),
        "admission precedes project creation"
    );
    assert!(!invalid_output.exists(), "admission precedes publication");

    let direct = directory.path().join("direct.json");
    Command::new(&binary)
        .env("MARKLAB_RUNTIME_ROOT", env!("CARGO_MANIFEST_DIR"))
        .args([
            "bayes",
            "test-cell-patch-complementarity",
            "--input",
            input.to_str().expect("input path"),
            "--outer-folds",
            "4",
            "--inner-folds",
            "3",
            "--ridge-alphas",
            "0,0.01,0.1,1,10",
            "--permutations",
            "99",
            "--seed",
            "1409",
            "--timeout-seconds",
            "120",
            "--out",
            direct.to_str().expect("direct output"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(&binary, &project, &input, &first, "99")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&binary, &project, &input, &replay, "99")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let direct_bytes = fs::read(&direct).expect("direct result");
    assert_eq!(direct_bytes, fs::read(&first).expect("project result"));
    assert_eq!(direct_bytes, fs::read(&replay).expect("replayed result"));
    let result: serde_json::Value = serde_json::from_slice(&direct_bytes).expect("result JSON");
    assert_eq!(result["backend"]["name"], "scipy");
    assert_eq!(result["backend"]["version"], "1.18.1");
    assert_eq!(result["backend"]["python_version"], "3.12");
    assert_eq!(result["patient_count"], 24);
    assert_eq!(result["models"].as_array().expect("models").len(), 6);

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "a replay must not append an execution");
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    let identity = &record["identity"];
    assert_eq!(identity["node"]["id"], "scipy-cell-patch-complementarity");
    let node = NodeSpec::new(
        NodeId::new("scipy-cell-patch-complementarity").expect("node ID"),
        "scipy_cell_patch_complementarity",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.cell_patch_complementarity"
    );
    assert_eq!(identity["result_schema"]["version"], 1);
    assert_eq!(identity["inputs"].as_array().expect("inputs").len(), 3);
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lock = repository.join("workers/python/uv.lock");
    let worker =
        repository.join("workers/python/marklab_scipy_cell_patch_complementarity_worker.py");
    for (index, path) in [input.as_path(), lock.as_path(), worker.as_path()]
        .into_iter()
        .enumerate()
    {
        let bytes = fs::read(path).expect("source");
        assert_eq!(
            identity["inputs"][index]["digest"],
            ContentDigest::from_bytes(&bytes).to_string()
        );
        assert_eq!(identity["inputs"][index]["byte_len"], bytes.len() as u64);
    }
    let request_sha256 = result["request_sha256"].as_str().expect("request digest");
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-cell-patch-complementarity-configuration-v1".as_slice(),
        request_sha256.as_bytes(),
        b"99".as_slice(),
        b"1409".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    assert_eq!(identity["runtime"]["backend"], "native");
    let executable = fs::read(&binary).expect("executable");
    assert_eq!(
        identity["runtime"]["executable"]["digest"],
        ContentDigest::from_bytes(&executable).to_string()
    );
}

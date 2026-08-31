#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

fn fixture(directory: &Path) -> std::path::PathBuf {
    let input = directory.join("nested_endpoints.csv");
    fs::write(
        &input,
        "patient_id,specimen_id,endpoint\n\
p-1,s-1,1\n\
p-1,s-2,3\n\
p-2,s-3,5\n\
p-2,s-4,7\n\
p-2,s-5,9\n\
p-3,s-6,2\n\
p-4,s-7,4\n\
p-4,s-8,6\n",
    )
    .expect("fixture");
    input
}

fn project_command(
    binary: &Path,
    project: &Path,
    input: &Path,
    output: &Path,
    maximum_patients: &str,
    maximum_specimens: &str,
    maximum_draws: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "cohort-hierarchical-bootstrap",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--replicates",
        "19",
        "--seed",
        "59",
        "--alpha",
        "0.05",
        "--maximum-patients",
        maximum_patients,
        "--maximum-specimens",
        maximum_specimens,
        "--maximum-bootstrap-draws",
        maximum_draws,
        "--memory-budget-mib",
        "8",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn patient_first_hierarchical_bootstrap_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable test executable");
    let input = fixture(directory.path());

    for (name, patients, specimens, draws, message) in [
        (
            "patient-limit",
            "3",
            "8",
            "228",
            "patient count 4 exceeds maximum_patients 3",
        ),
        (
            "specimen-limit",
            "4",
            "7",
            "228",
            "specimen count 8 exceeds maximum_specimens 7",
        ),
        (
            "draw-limit",
            "4",
            "8",
            "227",
            "228 bootstrap draws exceed maximum_bootstrap_draws 227",
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let output = directory.path().join(format!("{name}.json"));
        project_command(
            &binary, &project, &input, &output, patients, specimens, draws,
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains(message));
        assert!(!project.exists(), "{name} must precede project creation");
        assert!(!output.exists(), "{name} must precede publication");
    }

    let direct = directory.path().join("direct.json");
    Command::new(&binary)
        .args([
            "cohort",
            "hierarchical-bootstrap",
            "--input",
            input.to_str().expect("input path"),
            "--replicates",
            "19",
            "--seed",
            "59",
            "--alpha",
            "0.05",
            "--out",
            direct.to_str().expect("direct result"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(&binary, &project, &input, &first, "4", "8", "228")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&binary, &project, &input, &replay, "4", "8", "228")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let direct_bytes = fs::read(&direct).expect("direct result");
    assert_eq!(direct_bytes, fs::read(&first).expect("project result"));
    assert_eq!(direct_bytes, fs::read(&replay).expect("replayed result"));

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "a replay must not append an execution");
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    let identity = &record["identity"];
    assert_eq!(identity["node"]["id"], "cohort-hierarchical-bootstrap");
    let node = NodeSpec::new(
        NodeId::new("cohort-hierarchical-bootstrap").expect("node ID"),
        "cohort_hierarchical_bootstrap",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.cohort_hierarchical_bootstrap"
    );
    assert_eq!(identity["result_schema"]["version"], 1);
    let source = fs::read(&input).expect("source");
    assert_eq!(identity["inputs"].as_array().expect("inputs").len(), 1);
    assert_eq!(
        identity["inputs"][0]["digest"],
        ContentDigest::from_bytes(&source).to_string()
    );
    assert_eq!(identity["inputs"][0]["byte_len"], source.len() as u64);

    let input_path_json = serde_json::to_vec(&input).expect("path JSON");
    let alpha_bits = 0.05_f64.to_bits().to_string();
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-cohort-hierarchical-bootstrap-configuration-v1".as_slice(),
        input_path_json.as_slice(),
        b"19".as_slice(),
        b"59".as_slice(),
        alpha_bits.as_bytes(),
        b"4".as_slice(),
        b"8".as_slice(),
        b"228".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = format!(
        "serial;patient-then-nested-specimen-bootstrap;nearest-rank-percentile;replicates=19;seed=59;alpha_bits={alpha_bits};maximum_patients=4;maximum_specimens=8;maximum_bootstrap_draws=228;memory_budget_mib=8"
    );
    assert_eq!(
        identity["execution_policy_digest"],
        ContentDigest::from_bytes(execution_policy.as_bytes()).to_string()
    );
    assert_eq!(identity["runtime"]["backend"], "native");
    let executable = fs::read(&binary).expect("stable executable");
    assert_eq!(
        identity["runtime"]["executable"]["digest"],
        ContentDigest::from_bytes(&executable).to_string()
    );
    assert_eq!(
        identity["runtime"]["executable"]["byte_len"],
        executable.len() as u64
    );
}

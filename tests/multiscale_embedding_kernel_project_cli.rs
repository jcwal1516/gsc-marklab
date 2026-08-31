#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

#[path = "support/runtime_features.rs"]
mod runtime_features;

fn fixture(directory: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let input = directory.join("summaries.csv");
    let weights = directory.join("weights.csv");
    fs::write(
        &input,
        "sample_id,scale_um,embedding_0,embedding_1\na,10,1,0\na,20,0,2\nb,10,3,0\nb,20,0,1\n",
    )
    .expect("summary fixture");
    fs::write(&weights, "scale_um,weight\n10,0.25\n20,0.75\n").expect("weight fixture");
    (input, weights)
}

#[allow(clippy::too_many_arguments)]
fn project_command(
    binary: &Path,
    project: &Path,
    input: &Path,
    weights: &Path,
    output: &Path,
    maximum_summaries: &str,
    maximum_dimension: &str,
    maximum_work: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "multiscale-embedding-kernel",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--weights",
        weights.to_str().expect("weights path"),
        "--sample-a",
        "a",
        "--sample-b",
        "b",
        "--base-kernel",
        "linear",
        "--maximum-summaries",
        maximum_summaries,
        "--maximum-dimension",
        maximum_dimension,
        "--maximum-component-scale-visits",
        maximum_work,
        "--memory-budget-mib",
        "8",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn multiscale_kernel_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable test executable");
    let (input, weights) = fixture(directory.path());

    for (name, summaries, dimension, work, message) in [
        (
            "summary-limit",
            "3",
            "2",
            "4",
            "multiscale summary count exceeds maximum_summaries 3",
        ),
        (
            "dimension-limit",
            "4",
            "1",
            "4",
            "multiscale embedding dimension exceeds maximum_dimension 1",
        ),
        (
            "work-limit",
            "4",
            "2",
            "3",
            "component-scale work exceeds its bound",
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let output = directory.path().join(format!("{name}.json"));
        project_command(
            &binary, &project, &input, &weights, &output, summaries, dimension, work,
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains(message));
        assert!(!project.exists(), "{name} must precede project creation");
        assert!(!output.exists(), "{name} must precede publication");
    }

    let invalid_weights = directory.path().join("invalid-weights.csv");
    fs::write(&invalid_weights, "scale_um,weight\n10,0.25\n20,0.50\n").expect("invalid weights");
    let invalid_project = directory.path().join("invalid-project");
    let invalid_output = directory.path().join("invalid.json");
    project_command(
        &binary,
        &invalid_project,
        &input,
        &invalid_weights,
        &invalid_output,
        "4",
        "2",
        "4",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains("scale weights must sum to one"));
    assert!(!invalid_project.exists());
    assert!(!invalid_output.exists());

    let direct = directory.path().join("direct.json");
    Command::new(&binary)
        .args([
            "bayes",
            "multiscale-embedding-kernel",
            "--input",
            input.to_str().expect("input path"),
            "--weights",
            weights.to_str().expect("weights path"),
            "--sample-a",
            "a",
            "--sample-b",
            "b",
            "--base-kernel",
            "linear",
            "--maximum-component-scale-visits",
            "4",
            "--out",
            direct.to_str().expect("direct result"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(&binary, &project, &input, &weights, &first, "4", "2", "4")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&binary, &project, &input, &weights, &replay, "4", "2", "4")
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
    assert_eq!(identity["node"]["id"], "multiscale-embedding-kernel");
    let node = NodeSpec::new(
        NodeId::new("multiscale-embedding-kernel").expect("node ID"),
        "multiscale_embedding_kernel",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.multiscale_embedding_kernel"
    );
    assert_eq!(identity["result_schema"]["version"], 1);
    assert_eq!(identity["scheduler_output_limit_bytes"], 1024 * 1024);
    assert_eq!(identity["inputs"].as_array().expect("inputs").len(), 2);
    for (index, path) in [&input, &weights].into_iter().enumerate() {
        let bytes = fs::read(path).expect("source");
        assert_eq!(
            identity["inputs"][index]["digest"],
            ContentDigest::from_bytes(&bytes).to_string()
        );
        assert_eq!(identity["inputs"][index]["byte_len"], bytes.len() as u64);
    }
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-multiscale-embedding-kernel-configuration-v1".as_slice(),
        b"a".as_slice(),
        b"b".as_slice(),
        b"linear".as_slice(),
        b"none".as_slice(),
        b"4".as_slice(),
        b"2".as_slice(),
        b"4".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = b"serial;caller-prespecified-positive-sum-one-scales;drop-one-scale-sensitivity;base_kernel=linear;kernel_scale_bits=none;sample_a=a;sample_b=b;maximum_summaries=4;maximum_dimension=2;maximum_component_scale_visits=4;memory_budget_mib=8";
    assert_eq!(
        identity["execution_policy_digest"],
        ContentDigest::from_bytes(execution_policy).to_string()
    );
    assert_eq!(identity["runtime"]["backend"], "native");
    assert_eq!(
        identity["runtime"]["features"],
        runtime_features::expected_runtime_features()
    );
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

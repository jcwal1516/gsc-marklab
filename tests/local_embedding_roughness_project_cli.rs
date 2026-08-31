#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

fn fixture(directory: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let nodes = directory.join("nodes.csv");
    let edges = directory.join("edges.csv");
    fs::write(
        &nodes,
        "node_id,signal_0,signal_1\na,0,0\nb,1,0\nc,3,4\nd,9,9\n",
    )
    .expect("nodes");
    fs::write(&edges, "left_node_id,right_node_id,weight\na,b,1\nb,c,2\n").expect("edges");
    (nodes, edges)
}

#[allow(clippy::too_many_arguments)]
fn project_command(
    binary: &Path,
    project: &Path,
    nodes: &Path,
    edges: &Path,
    output: &Path,
    maximum_nodes: &str,
    maximum_edges: &str,
    maximum_dimension: &str,
    maximum_visits: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "local-embedding-roughness",
        "--project",
        project.to_str().expect("project path"),
        "--nodes",
        nodes.to_str().expect("nodes path"),
        "--edges",
        edges.to_str().expect("edges path"),
        "--epsilon",
        "0.000000000001",
        "--maximum-nodes",
        maximum_nodes,
        "--maximum-edges",
        maximum_edges,
        "--maximum-dimension",
        maximum_dimension,
        "--maximum-component-edge-visits",
        maximum_visits,
        "--memory-budget-mib",
        "8",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn local_embedding_roughness_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable executable");
    let (nodes, edges) = fixture(directory.path());

    for (name, max_nodes, max_edges, max_dimension, max_visits, message) in [
        (
            "node-limit",
            "3",
            "2",
            "2",
            "4",
            "node count exceeds maximum_nodes 3",
        ),
        (
            "edge-limit",
            "4",
            "1",
            "2",
            "4",
            "edge count exceeds maximum_edges 1",
        ),
        (
            "dimension-limit",
            "4",
            "2",
            "1",
            "4",
            "signal dimension exceeds maximum_dimension 1",
        ),
        (
            "work-limit",
            "4",
            "2",
            "2",
            "3",
            "4 component-edge visits exceed the declared or fixed resource bound",
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let output = directory.path().join(format!("{name}.json"));
        project_command(
            &binary,
            &project,
            &nodes,
            &edges,
            &output,
            max_nodes,
            max_edges,
            max_dimension,
            max_visits,
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains(message));
        assert!(!project.exists(), "{name} must precede project creation");
        assert!(!output.exists(), "{name} must precede publication");
    }

    let invalid_edges = directory.path().join("invalid-edges.csv");
    fs::write(
        &invalid_edges,
        "left_node_id,right_node_id,weight\na,missing,1\n",
    )
    .expect("invalid edges");
    let invalid_project = directory.path().join("invalid-project");
    let invalid_output = directory.path().join("invalid.json");
    project_command(
        &binary,
        &invalid_project,
        &nodes,
        &invalid_edges,
        &invalid_output,
        "4",
        "2",
        "2",
        "4",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "edge references unknown node missing",
    ));
    assert!(!invalid_project.exists());
    assert!(!invalid_output.exists());

    let direct = directory.path().join("direct.json");
    Command::new(&binary)
        .args([
            "bayes",
            "local-embedding-roughness",
            "--nodes",
            nodes.to_str().expect("nodes path"),
            "--edges",
            edges.to_str().expect("edges path"),
            "--epsilon",
            "0.000000000001",
            "--maximum-component-edge-visits",
            "4",
            "--out",
            direct.to_str().expect("direct output"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(
        &binary, &project, &nodes, &edges, &first, "4", "2", "2", "4",
    )
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=miss"));
    project_command(
        &binary, &project, &nodes, &edges, &replay, "4", "2", "2", "4",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=hit"));

    let direct_bytes = fs::read(&direct).expect("direct result");
    assert_eq!(direct_bytes, fs::read(&first).expect("project result"));
    assert_eq!(direct_bytes, fs::read(&replay).expect("replay result"));

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "a replay must not append an execution");
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    let identity = &record["identity"];
    assert_eq!(identity["node"]["id"], "local-embedding-roughness");
    let node = NodeSpec::new(
        NodeId::new("local-embedding-roughness").expect("node ID"),
        "local_embedding_roughness",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.local_embedding_roughness"
    );
    assert_eq!(identity["result_schema"]["version"], 1);
    assert_eq!(identity["inputs"].as_array().expect("inputs").len(), 2);
    for (index, path) in [&nodes, &edges].into_iter().enumerate() {
        let bytes = fs::read(path).expect("source");
        assert_eq!(
            identity["inputs"][index]["digest"],
            ContentDigest::from_bytes(&bytes).to_string()
        );
        assert_eq!(identity["inputs"][index]["byte_len"], bytes.len() as u64);
    }
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-local-embedding-roughness-configuration-v1".as_slice(),
        0.000000000001_f64.to_bits().to_string().as_bytes(),
        b"4".as_slice(),
        b"2".as_slice(),
        b"2".as_slice(),
        b"4".as_slice(),
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

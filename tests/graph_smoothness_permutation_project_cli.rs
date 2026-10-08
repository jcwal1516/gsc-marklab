#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

#[path = "support/runtime_features.rs"]
mod runtime_features;

fn fixture(directory: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let nodes = directory.join("nodes.csv");
    let edges = directory.join("edges.csv");
    fs::write(
        &nodes,
        "node_id,permutation_stratum,signal_0,signal_1\n\
n0,a,0,0\n\
n1,a,1,-1\n\
n2,a,2,-2\n\
n3,b,3,-3\n\
n4,b,4,-4\n\
n5,b,5,-5\n",
    )
    .expect("node fixture");
    fs::write(
        &edges,
        "left_node_id,right_node_id,weight\nn0,n1,1\nn1,n2,1\nn2,n3,1\nn3,n4,1\nn4,n5,1\n",
    )
    .expect("edge fixture");
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
    maximum_work: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "graph-smoothness-permutation-test",
        "--project",
        project.to_str().expect("project path"),
        "--nodes",
        nodes.to_str().expect("nodes path"),
        "--edges",
        edges.to_str().expect("edges path"),
        "--laplacian",
        "combinatorial",
        "--permutations",
        "20",
        "--seed",
        "1201",
        "--maximum-nodes",
        maximum_nodes,
        "--maximum-edges",
        maximum_edges,
        "--maximum-dimension",
        maximum_dimension,
        "--maximum-component-edge-visits",
        maximum_work,
        "--memory-budget-mib",
        "8",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn graph_smoothness_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable test executable");
    let (nodes, edges) = fixture(directory.path());

    for (name, max_nodes, max_edges, max_dimension, max_work, message) in [
        (
            "node-limit",
            "5",
            "5",
            "2",
            "210",
            "graph node count exceeds maximum_nodes 5",
        ),
        (
            "edge-limit",
            "6",
            "4",
            "2",
            "210",
            "graph edge count exceeds maximum_edges 4",
        ),
        (
            "dimension-limit",
            "6",
            "5",
            "1",
            "210",
            "graph signal dimension exceeds maximum_dimension 1",
        ),
        (
            "work-limit",
            "6",
            "5",
            "2",
            "209",
            "210 component-edge visits exceed the declared or fixed resource bound",
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
            max_work,
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
        "left_node_id,right_node_id,weight\nn0,n1,1\nn1,n0,1\n",
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
        "6",
        "2",
        "2",
        "84",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "graph edges must be unique unordered positive-weight non-self pairs",
    ));
    assert!(!invalid_project.exists());
    assert!(!invalid_output.exists());

    let direct = directory.path().join("direct.json");
    Command::new(&binary)
        .args([
            "bayes",
            "graph-smoothness-permutation-test",
            "--nodes",
            nodes.to_str().expect("nodes path"),
            "--edges",
            edges.to_str().expect("edges path"),
            "--laplacian",
            "combinatorial",
            "--permutations",
            "20",
            "--seed",
            "1201",
            "--maximum-component-edge-visits",
            "210",
            "--out",
            direct.to_str().expect("direct result"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(
        &binary, &project, &nodes, &edges, &first, "6", "5", "2", "210",
    )
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=miss"));
    project_command(
        &binary, &project, &nodes, &edges, &replay, "6", "5", "2", "210",
    )
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
    assert_eq!(identity["node"]["id"], "graph-smoothness-permutation-test");
    let node = NodeSpec::new(
        NodeId::new("graph-smoothness-permutation-test").expect("node ID"),
        "graph_smoothness_permutation_test",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.graph_smoothness_permutation_test"
    );
    assert_eq!(identity["result_schema"]["version"], 1);
    assert_eq!(identity["scheduler_output_limit_bytes"], 1024 * 1024);
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
        b"marklab-graph-smoothness-permutation-configuration-v1".as_slice(),
        b"combinatorial".as_slice(),
        b"20".as_slice(),
        b"1201".as_slice(),
        b"6".as_slice(),
        b"5".as_slice(),
        b"2".as_slice(),
        b"210".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = b"serial;complete-signal-rows-within-declared-strata;low-energy-inclusive-permutation-test;laplacian=combinatorial;normalization=signal;permutations=20;seed=1201;maximum_nodes=6;maximum_edges=5;maximum_dimension=2;maximum_component_edge_visits=210;memory_budget_mib=8";
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

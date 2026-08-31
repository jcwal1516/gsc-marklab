#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::ContentDigest;

#[path = "support/runtime_features.rs"]
mod runtime_features;

const POINTS: &str = "object_id,x_um,y_um,embedding_0,embedding_1,embedding_2\n\
a,0,0,0,0,1\n\
b,1,0,2,0,1\n\
c,3,0,2,2,4\n";
const BINS: &str = "bin_id,lower_um,upper_um\nnear,0,2\nfar,2,4\n";

fn project_command(
    project: &Path,
    points: &Path,
    bins: &Path,
    output: &Path,
    maximum_points: &str,
    maximum_dimension: &str,
    maximum_matrix_elements: &str,
) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "embedding-cross-covariance-by-distance",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        points.to_str().expect("points path"),
        "--bins",
        bins.to_str().expect("bins path"),
        "--out",
        output.to_str().expect("output path"),
        "--maximum-points",
        maximum_points,
        "--maximum-dimension",
        maximum_dimension,
        "--maximum-pair-visits",
        "3",
        "--maximum-matrix-elements",
        maximum_matrix_elements,
        "--memory-budget-mib",
        "16",
    ]);
    command
}

#[test]
fn embedding_cross_covariance_is_bounded_then_replays_the_direct_typed_result() {
    let directory = tempfile::tempdir().expect("tempdir");
    let points = directory.path().join("points.csv");
    let bins = directory.path().join("bins.csv");
    fs::write(&points, POINTS).expect("points");
    fs::write(&bins, BINS).expect("bins");

    for (name, maximum_points, maximum_dimension, maximum_matrix_elements, message) in [
        (
            "point-limit",
            "2",
            "3",
            "27",
            "point count exceeds maximum_points 2",
        ),
        (
            "dimension-limit",
            "3",
            "2",
            "27",
            "embedding dimension exceeds maximum_dimension 2",
        ),
        (
            "matrix-work-limit",
            "3",
            "3",
            "26",
            "27 pair-matrix element operations exceed maximum_matrix_elements 26",
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let output = directory.path().join(format!("{name}.json"));
        project_command(
            &project,
            &points,
            &bins,
            &output,
            maximum_points,
            maximum_dimension,
            maximum_matrix_elements,
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains(message));
        assert!(
            !project.exists(),
            "{name} must fail before project creation"
        );
        assert!(!output.exists(), "{name} must fail before publication");
    }

    let direct = directory.path().join("direct.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "embedding-cross-covariance-by-distance",
            "--input",
            points.to_str().unwrap(),
            "--bins",
            bins.to_str().unwrap(),
            "--maximum-pair-visits",
            "3",
            "--maximum-matrix-elements",
            "27",
            "--out",
            direct.to_str().unwrap(),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(&project, &points, &bins, &first, "3", "3", "27")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &points, &bins, &replay, "3", "3", "27")
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
    assert_eq!(identity["node"]["id"], "embedding-cross-covariance");
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.embedding_cross_covariance"
    );
    assert_eq!(identity["result_schema"]["version"], 1);
    assert_eq!(identity["scheduler_output_limit_bytes"], 1024 * 1024);
    assert_eq!(identity["inputs"].as_array().unwrap().len(), 2);
    for (index, path) in [&points, &bins].into_iter().enumerate() {
        let bytes = fs::read(path).expect("source");
        assert_eq!(
            identity["inputs"][index]["digest"],
            ContentDigest::from_bytes(&bytes).to_string()
        );
        assert_eq!(identity["inputs"][index]["byte_len"], bytes.len() as u64);
    }
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-embedding-cross-covariance-configuration-v1".as_slice(),
        b"3".as_slice(),
        b"3".as_slice(),
        b"3".as_slice(),
        b"27".as_slice(),
        b"16".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = b"serial;complete-vector-cross-covariance;physical-distance-bins;undirected-symmetrization;compensated-sums;maximum_points=3;maximum_dimension=3;maximum_pair_visits=3;maximum_matrix_elements=27;memory_budget_mib=16";
    assert_eq!(
        identity["execution_policy_digest"],
        ContentDigest::from_bytes(execution_policy).to_string()
    );
    assert_eq!(identity["runtime"]["backend"], "native");
    assert_eq!(
        identity["runtime"]["features"],
        runtime_features::expected_runtime_features()
    );
    let executable = fs::read(env!("CARGO_BIN_EXE_marklab")).expect("marklab executable");
    assert_eq!(
        identity["runtime"]["executable"]["digest"],
        ContentDigest::from_bytes(&executable).to_string()
    );
    assert_eq!(
        identity["runtime"]["executable"]["byte_len"],
        executable.len() as u64
    );
}

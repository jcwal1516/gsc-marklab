#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

fn fixture(path: &Path) {
    fs::write(
        path,
        "patient_id,group,feature,value,block\n\
a-1,A,x,3,north\n\
a-2,A,x,5,north\n\
b-1,B,x,0,north\n\
b-2,B,x,1,north\n\
a-3,A,x,4,south\n\
a-4,A,x,6,south\n\
b-3,B,x,1,south\n\
b-4,B,x,2,south\n",
    )
    .expect("fixture");
}

fn project_command(
    binary: &Path,
    project: &Path,
    input: &Path,
    output: &Path,
    maximum_evaluations: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "cohort-energy",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--group-a",
        "A",
        "--group-b",
        "B",
        "--metric",
        "euclidean",
        "--permutations",
        "99",
        "--seed",
        "53",
        "--maximum-patients",
        "8",
        "--maximum-features",
        "1",
        "--maximum-distance-elements",
        "64",
        "--maximum-energy-evaluations",
        maximum_evaluations,
        "--memory-budget-mib",
        "8",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn blocked_cohort_energy_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable executable");
    let input = directory.path().join("fingerprints.csv");
    fixture(&input);

    let invalid_project = directory.path().join("invalid-project");
    let invalid_output = directory.path().join("invalid.json");
    project_command(&binary, &invalid_project, &input, &invalid_output, "6399")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "6400 energy evaluations exceed maximum_energy_evaluations 6399",
        ));
    assert!(
        !invalid_project.exists(),
        "admission precedes project creation"
    );
    assert!(!invalid_output.exists(), "admission precedes publication");

    let direct = directory.path().join("direct.json");
    Command::new(&binary)
        .args([
            "cohort",
            "energy",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--metric",
            "euclidean",
            "--permutations",
            "99",
            "--seed",
            "53",
            "--out",
            direct.to_str().expect("direct output"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(&binary, &project, &input, &first, "6400")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&binary, &project, &input, &replay, "6400")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let direct_bytes = fs::read(&direct).expect("direct result");
    assert_eq!(direct_bytes, fs::read(&first).expect("project result"));
    assert_eq!(direct_bytes, fs::read(&replay).expect("replayed result"));
    let result: serde_json::Value = serde_json::from_slice(&direct_bytes).expect("result JSON");
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["design"]["blocked"], true);
    assert_eq!(result["design"]["block_count"], 2);
    assert_eq!(result["design"]["null_family"], "population_independence");
    assert_eq!(result["groups"]["group_a_patients"], 4);
    assert_eq!(result["groups"]["group_b_patients"], 4);

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "a replay must not append an execution");
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    let identity = &record["identity"];
    assert_eq!(identity["node"]["id"], "cohort-energy");
    let node = NodeSpec::new(
        NodeId::new("cohort-energy").expect("node ID"),
        "cohort_energy",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(identity["result_schema"]["id"], "marklab.cohort_energy");
    assert_eq!(identity["result_schema"]["version"], 1);
    assert_eq!(identity["inputs"].as_array().expect("inputs").len(), 1);
    let input_bytes = fs::read(&input).expect("input");
    assert_eq!(
        identity["inputs"][0]["digest"],
        ContentDigest::from_bytes(&input_bytes).to_string()
    );
    assert_eq!(identity["inputs"][0]["byte_len"], input_bytes.len() as u64);
    let input_path_identity = serde_json::to_vec(&input).expect("path identity");
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-cohort-energy-configuration-v1".as_slice(),
        input_path_identity.as_slice(),
        b"A".as_slice(),
        b"B".as_slice(),
        b"euclidean".as_slice(),
        b"99".as_slice(),
        b"53".as_slice(),
        b"true".as_slice(),
        b"8".as_slice(),
        b"1".as_slice(),
        b"64".as_slice(),
        b"6400".as_slice(),
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

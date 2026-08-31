#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

#[path = "support/runtime_features.rs"]
mod runtime_features;

fn fixture(directory: &Path) -> std::path::PathBuf {
    let input = directory.join("fingerprints.csv");
    fs::write(
        &input,
        "patient_id,group,feature,value,block\n\
a-1,A,x,3,north\na-1,A,y,0,north\n\
a-2,A,x,5,north\na-2,A,y,0,north\n\
b-1,B,x,0,north\nb-1,B,y,0,north\n\
b-2,B,x,1,north\nb-2,B,y,0,north\n\
a-3,A,x,4,south\na-3,A,y,1,south\n\
a-4,A,x,6,south\na-4,A,y,1,south\n\
b-3,B,x,1,south\nb-3,B,y,1,south\n\
b-4,B,x,2,south\nb-4,B,y,1,south\n",
    )
    .expect("fingerprint fixture");
    input
}

#[allow(clippy::too_many_arguments)]
fn project_command(
    binary: &Path,
    project: &Path,
    input: &Path,
    output: &Path,
    maximum_patients: &str,
    maximum_features: &str,
    maximum_kernel_elements: &str,
    maximum_mmd_evaluations: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "cohort-mmd",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--group-a",
        "A",
        "--group-b",
        "B",
        "--kernel",
        "linear",
        "--estimator",
        "unbiased",
        "--permutations",
        "19",
        "--seed",
        "47",
        "--maximum-patients",
        maximum_patients,
        "--maximum-features",
        maximum_features,
        "--maximum-kernel-elements",
        maximum_kernel_elements,
        "--maximum-mmd-evaluations",
        maximum_mmd_evaluations,
        "--memory-budget-mib",
        "8",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn blocked_cohort_mmd_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable test executable");
    let input = fixture(directory.path());

    for (name, patients, features, kernel_elements, work, message) in [
        (
            "patient-limit",
            "7",
            "2",
            "64",
            "1280",
            "cohort MMD patient count exceeds maximum_patients 7",
        ),
        (
            "feature-limit",
            "8",
            "1",
            "64",
            "1280",
            "within maximum_features 1",
        ),
        (
            "kernel-limit",
            "8",
            "2",
            "63",
            "1280",
            "64 elements, exceeding maximum_kernel_elements 63",
        ),
        (
            "work-limit",
            "8",
            "2",
            "64",
            "1279",
            "1280 kernel-by-permutation evaluations, exceeding maximum_mmd_evaluations 1279",
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let output = directory.path().join(format!("{name}.json"));
        project_command(
            &binary,
            &project,
            &input,
            &output,
            patients,
            features,
            kernel_elements,
            work,
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains(message));
        assert!(!project.exists(), "{name} must precede project creation");
        assert!(!output.exists(), "{name} must precede publication");
    }

    let confounded = directory.path().join("confounded.csv");
    fs::write(
        &confounded,
        fs::read_to_string(&input)
            .expect("fixture")
            .replace("b-1,B,x,0,north", "b-1,B,x,0,south")
            .replace("b-1,B,y,0,north", "b-1,B,y,0,south")
            .replace("b-2,B,x,1,north", "b-2,B,x,1,south")
            .replace("b-2,B,y,0,north", "b-2,B,y,0,south")
            .replace("a-3,A,x,4,south", "a-3,A,x,4,north")
            .replace("a-3,A,y,1,south", "a-3,A,y,1,north")
            .replace("a-4,A,x,6,south", "a-4,A,x,6,north")
            .replace("a-4,A,y,1,south", "a-4,A,y,1,north"),
    )
    .expect("confounded fixture");
    let confounded_project = directory.path().join("confounded-project");
    let confounded_output = directory.path().join("confounded.json");
    project_command(
        &binary,
        &confounded_project,
        &confounded,
        &confounded_output,
        "8",
        "2",
        "64",
        "1280",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "fingerprint blocks are fully confounded with group",
    ));
    assert!(!confounded_project.exists());
    assert!(!confounded_output.exists());

    let direct = directory.path().join("direct.json");
    Command::new(&binary)
        .args([
            "cohort",
            "mmd",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--kernel",
            "linear",
            "--estimator",
            "unbiased",
            "--permutations",
            "19",
            "--seed",
            "47",
            "--out",
            direct.to_str().expect("direct result"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(&binary, &project, &input, &first, "8", "2", "64", "1280")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&binary, &project, &input, &replay, "8", "2", "64", "1280")
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
    assert_eq!(identity["node"]["id"], "cohort-mmd");
    let node = NodeSpec::new(
        NodeId::new("cohort-mmd").expect("node ID"),
        "cohort_mmd",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(identity["result_schema"]["id"], "marklab.cohort_mmd");
    assert_eq!(identity["result_schema"]["version"], 1);
    assert_eq!(identity["scheduler_output_limit_bytes"], 1024 * 1024);
    let source = fs::read(&input).expect("source");
    assert_eq!(identity["inputs"].as_array().expect("inputs").len(), 1);
    assert_eq!(
        identity["inputs"][0]["digest"],
        ContentDigest::from_bytes(&source).to_string()
    );
    assert_eq!(identity["inputs"][0]["byte_len"], source.len() as u64);
    let input_path_json = serde_json::to_vec(&input).expect("path JSON");
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-cohort-mmd-configuration-v1".as_slice(),
        input_path_json.as_slice(),
        b"A".as_slice(),
        b"B".as_slice(),
        b"linear".as_slice(),
        b"none".as_slice(),
        b"unbiased".as_slice(),
        b"19".as_slice(),
        b"47".as_slice(),
        b"8".as_slice(),
        b"2".as_slice(),
        b"64".as_slice(),
        b"1280".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = b"serial;patient-label-permutation;optional-exact-exchangeability-blocks;inclusive-plus-one-high-tail;kernel=linear;bandwidth_bits=none;estimator=unbiased;permutations=19;seed=47;maximum_patients=8;maximum_features=2;maximum_kernel_elements=64;maximum_mmd_evaluations=1280;memory_budget_mib=8";
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

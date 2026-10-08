#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

#[path = "support/runtime_features.rs"]
mod runtime_features;

fn fixture(directory: &Path) -> std::path::PathBuf {
    let input = directory.join("endpoints.csv");
    let mut rows = String::from("patient_id,group,endpoint,value,block\n");
    for (patient, group, e1, e2, block) in [
        ("a-1", "A", 8, 4, "north"),
        ("a-2", "A", 9, 7, "north"),
        ("b-1", "B", 1, 2, "north"),
        ("b-2", "B", 2, 3, "north"),
        ("a-3", "A", 7, 5, "south"),
        ("a-4", "A", 10, 8, "south"),
        ("b-3", "B", 2, 1, "south"),
        ("b-4", "B", 3, 4, "south"),
    ] {
        rows.push_str(&format!("{patient},{group},e1,{e1},{block}\n"));
        rows.push_str(&format!("{patient},{group},e2,{e2},{block}\n"));
    }
    fs::write(&input, rows).expect("endpoint fixture");
    input
}

#[allow(clippy::too_many_arguments)]
fn project_command(
    binary: &Path,
    project: &Path,
    input: &Path,
    output: &Path,
    maximum_patients: &str,
    maximum_endpoints: &str,
    maximum_cells: &str,
    maximum_evaluations: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "cohort-max-t",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--group-a",
        "A",
        "--group-b",
        "B",
        "--permutations",
        "19",
        "--seed",
        "41",
        "--alpha",
        "0.05",
        "--step-down",
        "--maximum-patients",
        maximum_patients,
        "--maximum-endpoints",
        maximum_endpoints,
        "--maximum-patient-endpoint-cells",
        maximum_cells,
        "--maximum-permutation-endpoint-evaluations",
        maximum_evaluations,
        "--memory-budget-mib",
        "8",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn blocked_step_down_max_t_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable test executable");
    let input = fixture(directory.path());

    for (name, patients, endpoints, cells, evaluations, message) in [
        (
            "patient-limit",
            "7",
            "2",
            "16",
            "320",
            "cohort Max-T patient count exceeds maximum_patients 7",
        ),
        (
            "endpoint-limit",
            "8",
            "1",
            "16",
            "320",
            "within maximum_endpoints 1",
        ),
        (
            "cell-limit",
            "8",
            "2",
            "15",
            "320",
            "16 patient-endpoint cells, exceeding maximum_patient_endpoint_cells 15",
        ),
        (
            "work-limit",
            "8",
            "2",
            "16",
            "319",
            "320 patient-by-endpoint-by-permutation evaluations, exceeding maximum_permutation_endpoint_evaluations 319",
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
            endpoints,
            cells,
            evaluations,
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains(message));
        assert!(!project.exists(), "{name} must precede project creation");
        assert!(!output.exists(), "{name} must precede publication");
    }

    let confounded = directory.path().join("confounded.csv");
    let text = fs::read_to_string(&input).expect("fixture");
    let text = text
        .replace("b-1,B,e1,1,north", "b-1,B,e1,1,south")
        .replace("b-1,B,e2,2,north", "b-1,B,e2,2,south")
        .replace("b-2,B,e1,2,north", "b-2,B,e1,2,south")
        .replace("b-2,B,e2,3,north", "b-2,B,e2,3,south")
        .replace("a-3,A,e1,7,south", "a-3,A,e1,7,north")
        .replace("a-3,A,e2,5,south", "a-3,A,e2,5,north")
        .replace("a-4,A,e1,10,south", "a-4,A,e1,10,north")
        .replace("a-4,A,e2,8,south", "a-4,A,e2,8,north");
    fs::write(&confounded, text).expect("confounded fixture");
    let confounded_project = directory.path().join("confounded-project");
    let confounded_output = directory.path().join("confounded.json");
    project_command(
        &binary,
        &confounded_project,
        &confounded,
        &confounded_output,
        "8",
        "2",
        "16",
        "320",
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
            "max-t",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "19",
            "--seed",
            "41",
            "--alpha",
            "0.05",
            "--step-down",
            "--out",
            direct.to_str().expect("direct result"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(&binary, &project, &input, &first, "8", "2", "16", "320")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&binary, &project, &input, &replay, "8", "2", "16", "320")
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
    assert_eq!(identity["node"]["id"], "cohort-max-t");
    let node = NodeSpec::new(
        NodeId::new("cohort-max-t").expect("node ID"),
        "cohort_max_t",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(identity["result_schema"]["id"], "marklab.cohort_max_t");
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
        b"marklab-cohort-max-t-configuration-v1".as_slice(),
        input_path_json.as_slice(),
        b"A".as_slice(),
        b"B".as_slice(),
        b"19".as_slice(),
        b"41".as_slice(),
        alpha_bits.as_bytes(),
        b"step_down_max_t".as_slice(),
        b"8".as_slice(),
        b"2".as_slice(),
        b"16".as_slice(),
        b"320".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = format!(
        "serial;whole-patient-label-permutation;optional-exact-exchangeability-blocks;complete-endpoint-family;two-sided;correction=step_down_max_t;permutations=19;seed=41;alpha_bits={alpha_bits};maximum_patients=8;maximum_endpoints=2;maximum_patient_endpoint_cells=16;maximum_permutation_endpoint_evaluations=320;memory_budget_mib=8"
    );
    assert_eq!(
        identity["execution_policy_digest"],
        ContentDigest::from_bytes(execution_policy.as_bytes()).to_string()
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

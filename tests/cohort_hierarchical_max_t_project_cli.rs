#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

fn fixture(path: &Path) {
    let mut csv = String::from("patient_id,group,family_order,family,endpoint,value\n");
    let rows = [
        ("a1", "A", [8.0, 7.0, 5.0]),
        ("a2", "A", [9.0, 8.0, 6.0]),
        ("a3", "A", [10.0, 9.0, 7.0]),
        ("a4", "A", [11.0, 10.0, 8.0]),
        ("b1", "B", [1.0, 1.5, 4.0]),
        ("b2", "B", [2.0, 2.5, 4.5]),
        ("b3", "B", [3.0, 3.5, 5.0]),
        ("b4", "B", [4.0, 4.5, 5.5]),
    ];
    for (patient, group, values) in rows {
        csv.push_str(&format!(
            "{patient},{group},1,primary,response,{}\n",
            values[0]
        ));
        csv.push_str(&format!(
            "{patient},{group},1,primary,stability,{}\n",
            values[1]
        ));
        csv.push_str(&format!(
            "{patient},{group},2,secondary,mechanism,{}\n",
            values[2]
        ));
    }
    fs::write(path, csv).expect("fixture");
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
        "cohort-hierarchical-max-t",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--group-a",
        "A",
        "--group-b",
        "B",
        "--permutations",
        "199",
        "--seed",
        "43",
        "--alpha",
        "0.2",
        "--step-down",
        "--maximum-patients",
        "8",
        "--maximum-families",
        "2",
        "--maximum-endpoints",
        "3",
        "--maximum-cells",
        "24",
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
fn hierarchical_max_t_is_bounded_then_replays_exact_step_down_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable executable");
    let input = directory.path().join("hierarchical-max-t.csv");
    fixture(&input);

    let invalid_project = directory.path().join("invalid-project");
    let invalid_output = directory.path().join("invalid.json");
    project_command(
        &binary,
        &invalid_project,
        &input,
        &invalid_output,
        "4799",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "4800 patient-endpoint-permutation evaluations exceed maximum_permutation_endpoint_evaluations 4799",
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
            "hierarchical-max-t",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "199",
            "--seed",
            "43",
            "--alpha",
            "0.2",
            "--step-down",
            "--out",
            direct.to_str().expect("direct output"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(&binary, &project, &input, &first, "4800")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&binary, &project, &input, &replay, "4800")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let direct_bytes = fs::read(&direct).expect("direct result");
    assert_eq!(direct_bytes, fs::read(&first).expect("project result"));
    assert_eq!(direct_bytes, fs::read(&replay).expect("replayed result"));
    let result: serde_json::Value = serde_json::from_slice(&direct_bytes).expect("result JSON");
    assert_eq!(result["design"]["correction"], "step_down_max_t");
    assert_eq!(
        result["design"]["multiplicity"],
        "ordered_family_gatekeeping_max_t"
    );
    assert_eq!(result["families"][0]["family"], "primary");
    assert_eq!(result["families"][1]["family"], "secondary");
    assert_eq!(result["patients"]["total"], 8);

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "a replay must not append an execution");
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    let identity = &record["identity"];
    assert_eq!(identity["node"]["id"], "cohort-hierarchical-max-t");
    let node = NodeSpec::new(
        NodeId::new("cohort-hierarchical-max-t").expect("node ID"),
        "cohort_hierarchical_max_t",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.cohort_hierarchical_max_t"
    );
    assert_eq!(identity["result_schema"]["version"], 1);
    let input_bytes = fs::read(&input).expect("input");
    assert_eq!(
        identity["inputs"][0]["digest"],
        ContentDigest::from_bytes(&input_bytes).to_string()
    );
    let input_path_identity = serde_json::to_vec(&input).expect("path identity");
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-cohort-hierarchical-max-t-configuration-v1".as_slice(),
        input_path_identity.as_slice(),
        b"A".as_slice(),
        b"B".as_slice(),
        b"199".as_slice(),
        b"43".as_slice(),
        0.2_f64.to_bits().to_string().as_bytes(),
        b"step_down_max_t".as_slice(),
        b"8".as_slice(),
        b"2".as_slice(),
        b"3".as_slice(),
        b"24".as_slice(),
        b"4800".as_slice(),
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

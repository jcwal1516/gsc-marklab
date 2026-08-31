#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

fn fixture(directory: &Path) -> std::path::PathBuf {
    let input = directory.join("cluster_covariates.csv");
    fs::write(
        &input,
        "patient_id,cluster_id,group,outcome,covariate,value\n\
a1-1,a1,A,4.0,age,-3\n\
a1-1,a1,A,4.0,batch,-1\n\
a1-2,a1,A,6.0,age,-1\n\
a1-2,a1,A,6.0,batch,1\n\
a2,a2,A,5.0,age,-1\n\
a2,a2,A,5.0,batch,-1\n\
a3,a3,A,7.0,age,1\n\
a3,a3,A,7.0,batch,1\n\
a4,a4,A,7.0,age,3\n\
a4,a4,A,7.0,batch,-1\n\
b1,b1,B,0.0,age,-3\n\
b1,b1,B,0.0,batch,1\n\
b2,b2,B,2.0,age,-1\n\
b2,b2,B,2.0,batch,-1\n\
b3,b3,B,2.0,age,1\n\
b3,b3,B,2.0,batch,1\n\
b4-1,b4,B,3.0,age,2\n\
b4-1,b4,B,3.0,batch,-1\n\
b4-2,b4,B,5.0,age,4\n\
b4-2,b4,B,5.0,batch,1\n",
    )
    .expect("fixture");
    input
}

#[allow(clippy::too_many_arguments)]
fn project_command(
    binary: &Path,
    project: &Path,
    input: &Path,
    output: &Path,
    maximum_patients: &str,
    maximum_clusters: &str,
    maximum_covariates: &str,
    maximum_cells: &str,
    maximum_work: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "cohort-cluster-covariate-permutation",
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
        "71",
        "--alternative",
        "two-sided",
        "--maximum-patients",
        maximum_patients,
        "--maximum-clusters",
        maximum_clusters,
        "--maximum-covariates",
        maximum_covariates,
        "--maximum-patient-covariate-cells",
        maximum_cells,
        "--maximum-ols-work",
        maximum_work,
        "--memory-budget-mib",
        "8",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn adjusted_whole_cluster_inference_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable test executable");
    let input = fixture(directory.path());

    for (name, patients, clusters, covariates, cells, work, message) in [
        (
            "patient-limit",
            "9",
            "8",
            "2",
            "20",
            "4032",
            "patient count 10 exceeds maximum_patients 9",
        ),
        (
            "cluster-limit",
            "10",
            "7",
            "2",
            "20",
            "4032",
            "cluster count 8 exceeds maximum_clusters 7",
        ),
        (
            "covariate-limit",
            "10",
            "8",
            "1",
            "20",
            "4032",
            "covariate count 2 exceeds maximum_covariates 1",
        ),
        (
            "cell-limit",
            "10",
            "8",
            "2",
            "19",
            "4032",
            "20 patient-covariate cells exceed maximum_patient_covariate_cells 19",
        ),
        (
            "work-limit",
            "10",
            "8",
            "2",
            "20",
            "4031",
            "4032 OLS work units exceed maximum_ols_work 4031",
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let output = directory.path().join(format!("{name}.json"));
        project_command(
            &binary, &project, &input, &output, patients, clusters, covariates, cells, work,
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
            "cluster-covariate-permutation",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "19",
            "--seed",
            "71",
            "--alternative",
            "two-sided",
            "--out",
            direct.to_str().expect("direct result"),
        ])
        .assert()
        .success();

    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let replay = directory.path().join("replay.json");
    project_command(
        &binary, &project, &input, &first, "10", "8", "2", "20", "4032",
    )
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=miss"));
    project_command(
        &binary, &project, &input, &replay, "10", "8", "2", "20", "4032",
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
    assert_eq!(identity["node"]["id"], "cohort-cluster-covariate");
    let node = NodeSpec::new(
        NodeId::new("cohort-cluster-covariate").expect("node ID"),
        "cohort_cluster_covariate_permutation",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.cohort_cluster_covariate_permutation"
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
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-cohort-cluster-covariate-configuration-v1".as_slice(),
        input_path_json.as_slice(),
        b"A".as_slice(),
        b"B".as_slice(),
        b"19".as_slice(),
        b"71".as_slice(),
        b"two-sided".as_slice(),
        b"10".as_slice(),
        b"8".as_slice(),
        b"2".as_slice(),
        b"20".as_slice(),
        b"4032".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = b"serial;equal-weight-patient-cluster-summaries;complete-cluster-residual-permutation;alternative=two-sided;permutations=19;seed=71;maximum_patients=10;maximum_clusters=8;maximum_covariates=2;maximum_patient_covariate_cells=20;maximum_ols_work=4032;memory_budget_mib=8";
    assert_eq!(
        identity["execution_policy_digest"],
        ContentDigest::from_bytes(execution_policy).to_string()
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

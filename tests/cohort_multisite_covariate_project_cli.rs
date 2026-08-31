#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::{ContentDigest, NodeId, NodeSpec};

fn fixture(directory: &Path) -> std::path::PathBuf {
    let input = directory.join("patients.csv");
    let mut rows = String::from("patient_id,site_id,group,endpoint,covariate,value\n");
    let noise = [-1.0, 0.2, 0.8, -0.4];
    for (site_index, site) in ["site-a", "site-b", "site-c"].into_iter().enumerate() {
        for group in ["A", "B"] {
            for (index, noise) in noise.into_iter().enumerate() {
                let batch = (index % 2) as f64;
                let endpoint = site_index as f64
                    + 2.0 * index as f64
                    + 0.5 * batch
                    + noise
                    + if group == "A" { 3.0 } else { 0.0 };
                let patient = format!("{site}-{group}-{}", index + 1);
                rows.push_str(&format!(
                    "{patient},{site},{group},{endpoint},age,{index}\n"
                ));
                rows.push_str(&format!(
                    "{patient},{site},{group},{endpoint},batch,{batch}\n"
                ));
            }
        }
    }
    fs::write(&input, rows).expect("fixture");
    input
}

#[allow(clippy::too_many_arguments)]
fn project_command(
    binary: &Path,
    project: &Path,
    input: &Path,
    output: &Path,
    maximum_patients: &str,
    maximum_sites: &str,
    maximum_covariates: &str,
    maximum_cells: &str,
    maximum_work: &str,
) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "cohort-multisite-covariate-contrast",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--group-a",
        "A",
        "--group-b",
        "B",
        "--model",
        "fixed-effect",
        "--alpha",
        "0.05",
        "--maximum-patients",
        maximum_patients,
        "--maximum-sites",
        maximum_sites,
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
fn adjusted_multisite_contrast_is_bounded_then_replays_exact_direct_bytes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let binary = directory.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable test executable");
    let input = fixture(directory.path());

    for (name, patients, sites, covariates, cells, work, message) in [
        (
            "patient-limit",
            "23",
            "3",
            "2",
            "48",
            "576",
            "patient count 24 exceeds maximum_patients 23",
        ),
        (
            "site-limit",
            "24",
            "2",
            "2",
            "48",
            "576",
            "site count 3 exceeds maximum_sites 2",
        ),
        (
            "covariate-limit",
            "24",
            "3",
            "1",
            "48",
            "576",
            "covariate count 2 exceeds maximum_covariates 1",
        ),
        (
            "cell-limit",
            "24",
            "3",
            "2",
            "47",
            "576",
            "48 patient-covariate cells exceed maximum_patient_covariate_cells 47",
        ),
        (
            "work-limit",
            "24",
            "3",
            "2",
            "48",
            "575",
            "576 OLS work units exceed maximum_ols_work 575",
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let output = directory.path().join(format!("{name}.json"));
        project_command(
            &binary, &project, &input, &output, patients, sites, covariates, cells, work,
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
            "multisite-covariate-contrast",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--model",
            "fixed-effect",
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
    project_command(
        &binary, &project, &input, &first, "24", "3", "2", "48", "576",
    )
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=miss"));
    project_command(
        &binary, &project, &input, &replay, "24", "3", "2", "48", "576",
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
    assert_eq!(identity["node"]["id"], "cohort-multisite-covariate");
    let node = NodeSpec::new(
        NodeId::new("cohort-multisite-covariate").expect("node ID"),
        "cohort_multisite_covariate_contrast",
        1,
        Vec::new(),
    )
    .expect("node spec");
    assert_eq!(identity["node"]["spec_digest"], node.digest().to_string());
    assert_eq!(
        identity["result_schema"]["id"],
        "marklab.cohort_multisite_covariate_contrast"
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
        b"marklab-cohort-multisite-covariate-configuration-v1".as_slice(),
        input_path_json.as_slice(),
        b"A".as_slice(),
        b"B".as_slice(),
        b"fixed-effect".as_slice(),
        alpha_bits.as_bytes(),
        b"24".as_slice(),
        b"3".as_slice(),
        b"2".as_slice(),
        b"48".as_slice(),
        b"576".as_slice(),
        b"8".as_slice(),
    ]);
    assert_eq!(
        identity["configuration_digest"],
        configuration_digest.to_string()
    );
    let execution_policy = format!(
        "serial;patient-unit;within-site-adjusted-ols;inverse-variance-pooling;model=fixed-effect;alpha_bits={alpha_bits};maximum_patients=24;maximum_sites=3;maximum_covariates=2;maximum_patient_covariate_cells=48;maximum_ols_work=576;memory_budget_mib=8"
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

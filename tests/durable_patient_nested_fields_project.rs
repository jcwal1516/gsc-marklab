#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn write_input(path: &Path) {
    let mut rows = String::from("patient_id,specimen_id,group,endpoint,value\n");
    for (patient, group, first, second) in [
        ("a1", "A", 2.0, 0.0),
        ("a2", "A", 1.8, 0.2),
        ("b1", "B", 0.0, 2.0),
        ("b2", "B", 0.2, 1.8),
    ] {
        for (slide, offset) in [("s1", -0.05), ("s2", 0.05)] {
            rows.push_str(&format!(
                "{patient},{patient}-{slide},{group},local_mean_abs,{:.17}\n",
                first + offset
            ));
            rows.push_str(&format!(
                "{patient},{patient}-{slide},{group},spde_factor_sd,{:.17}\n",
                second - offset
            ));
        }
    }
    fs::write(path, rows).expect("write input");
}

fn project_command(binary: &Path, project: &Path, input: &Path, out: &Path, work: &str) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "project",
        "patient-nested-fields",
        "--project",
        project.to_str().expect("project"),
        "--input",
        input.to_str().expect("input"),
        "--group-a",
        "A",
        "--group-b",
        "B",
        "--permutations",
        "31",
        "--seed",
        "73",
        "--alpha",
        "0.05",
        "--maximum-patients",
        "4",
        "--maximum-specimens",
        "8",
        "--maximum-endpoints",
        "2",
        "--maximum-permutation-endpoint-evaluations",
        work,
        "--memory-budget-mib",
        "8",
        "--out",
        out.to_str().expect("out"),
    ]);
    command
}

#[test]
fn patient_nested_fields_replay_without_a_second_inference_execution() {
    let root = tempfile::tempdir().expect("root");
    let binary = root.path().join("marklab");
    fs::copy(env!("CARGO_BIN_EXE_marklab"), &binary).expect("stable binary");
    let input = root.path().join("fields.csv");
    write_input(&input);

    let invalid_project = root.path().join("invalid-project");
    project_command(
        &binary,
        &invalid_project,
        &input,
        &root.path().join("invalid.json"),
        "255",
    )
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "permutation endpoint evaluations 256 exceed maximum_permutation_endpoint_evaluations 255",
    ));
    assert!(!invalid_project.exists());

    let direct = root.path().join("direct.json");
    Command::new(&binary)
        .args([
            "cohort",
            "patient-nested-fields",
            "--input",
            input.to_str().unwrap(),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "31",
            "--seed",
            "73",
            "--alpha",
            "0.05",
            "--maximum-patients",
            "4",
            "--maximum-specimens",
            "8",
            "--maximum-endpoints",
            "2",
            "--maximum-permutation-endpoint-evaluations",
            "256",
            "--memory-budget-mib",
            "8",
            "--out",
            direct.to_str().unwrap(),
        ])
        .assert()
        .success();

    let project = root.path().join("project");
    let miss = root.path().join("miss.json");
    let hit = root.path().join("hit.json");
    project_command(&binary, &project, &input, &miss, "256")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&binary, &project, &input, &hit, "256")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let direct_bytes = fs::read(direct).expect("direct");
    assert_eq!(direct_bytes, fs::read(miss).expect("miss"));
    assert_eq!(direct_bytes, fs::read(hit).expect("hit"));
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .expect("ledger")
            .lines()
            .count(),
        1
    );
}

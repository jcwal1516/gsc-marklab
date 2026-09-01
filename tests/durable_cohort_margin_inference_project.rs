#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn equivalence(project: &Path, input: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "cohort-equivalence",
        "--project",
        project.to_str().expect("project"),
        "--input",
        input.to_str().expect("input"),
        "--lower-margin",
        "-0.2",
        "--upper-margin",
        "0.2",
        "--alpha",
        "0.05",
        "--margin-rationale",
        "protocol-margin-v1",
        "--out",
        out.to_str().expect("output"),
    ]);
    command
}

fn noninferiority(project: &Path, input: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "cohort-noninferiority",
        "--project",
        project.to_str().expect("project"),
        "--input",
        input.to_str().expect("input"),
        "--direction",
        "higher-is-better",
        "--margin",
        "0.2",
        "--alpha",
        "0.05",
        "--margin-rationale",
        "protocol-ni-margin-v1",
        "--out",
        out.to_str().expect("output"),
    ]);
    command
}

#[test]
fn declared_margin_inference_replays_without_recomputation() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("effects.csv");
    fs::write(
        &input,
        "patient_id,effect\np-1,-0.1\np-2,0\np-3,0.1\np-4,0\n",
    )
    .expect("fixture");
    for (name, builder) in [
        (
            "equivalence",
            equivalence as fn(&Path, &Path, &Path) -> Command,
        ),
        (
            "noninferiority",
            noninferiority as fn(&Path, &Path, &Path) -> Command,
        ),
    ] {
        let project = directory.path().join(format!("{name}-project"));
        let first = directory.path().join(format!("{name}-first.json"));
        let second = directory.path().join(format!("{name}-second.json"));
        builder(&project, &input, &first)
            .assert()
            .success()
            .stderr(predicates::str::contains("cache_status=miss"));
        builder(&project, &input, &second)
            .assert()
            .success()
            .stderr(predicates::str::contains("cache_status=hit"));
        assert_eq!(
            fs::read(first).expect("miss"),
            fs::read(second).expect("hit")
        );
        assert_eq!(
            fs::read_to_string(project.join("executions.jsonl"))
                .expect("ledger")
                .lines()
                .count(),
            1
        );
    }
}

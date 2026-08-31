#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

#[path = "support/joint_location_embedding_fixture.rs"]
mod fixture;

#[test]
fn joint_location_embedding_fit_replays_without_two_more_backend_fits() {
    let directory = tempfile::tempdir().expect("tempdir");
    let location = directory.path().join("location.csv");
    let embedding = directory.path().join("embedding.csv");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fixture::write_inputs(&location, &embedding);

    project_command(&project, &location, &embedding, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &location, &embedding, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

fn project_command(project: &Path, location: &Path, embedding: &Path, output: &Path) -> Command {
    let arguments = fixture::arguments(location, embedding, output);
    let mut project_command = Command::cargo_bin("marklab").expect("binary");
    project_command
        .arg("project")
        .arg("joint-replicated-location-embedding")
        .arg("--project")
        .arg(project)
        .args(arguments.into_iter().skip(2));
    project_command
}

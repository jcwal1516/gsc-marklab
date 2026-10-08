#![cfg(feature = "cli")]

#[path = "support/multiplex_study.rs"]
mod support;

use assert_cmd::Command;
use std::fs;

#[test]
fn the_cli_and_library_share_the_same_study_and_resume_across_processes() {
    let directory = tempfile::tempdir().unwrap();
    let recipe = directory.path().join("recipe.json");
    let bytes = serde_json::to_vec(&support::recipe()).unwrap();
    fs::write(&recipe, &bytes).unwrap();
    let project = directory.path().join("project");
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    let mut partial = Command::cargo_bin("marklab").unwrap();
    partial
        .args(["study", "run", "--recipe"])
        .arg(&recipe)
        .arg("--project")
        .arg(&project)
        .arg("--out")
        .arg(&first)
        .args(["--through-slides", "3"])
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("paused after 3 slides"));
    assert!(!first.exists());
    for (out, expected) in [
        (&first, "executed_slides=9 restored_slides=3"),
        (&second, "executed_slides=0 restored_slides=12"),
    ] {
        Command::cargo_bin("marklab")
            .unwrap()
            .args(["study", "run", "--recipe"])
            .arg(&recipe)
            .arg("--project")
            .arg(&project)
            .arg("--out")
            .arg(out)
            .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
            .assert()
            .success()
            .stderr(predicates::str::contains(expected));
    }
    let expected = marklab::analyze_multiplex_study(&bytes).unwrap();
    let mut expected = serde_json::to_vec_pretty(&expected).unwrap();
    expected.push(b'\n');
    assert_eq!(fs::read(first.join("result.json")).unwrap(), expected);
    assert_eq!(fs::read(second.join("result.json")).unwrap(), expected);
    assert_eq!(
        fs::read(project.join("executions.jsonl"))
            .unwrap()
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .count(),
        13
    );
    let help = Command::cargo_bin("marklab")
        .unwrap()
        .args(["study", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8(help).unwrap().contains("describe"));
}

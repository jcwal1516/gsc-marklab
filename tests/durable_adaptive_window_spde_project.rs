#![cfg(feature = "cli")]

use std::fs;

#[path = "support/adaptive_window_spde_fixture.rs"]
mod fixture;

#[test]
fn adaptive_window_spde_replays_without_a_second_backend_execution() {
    let root = tempfile::tempdir().expect("root");
    fixture::write_input(root.path());
    let project = root.path().join("project");
    let first = root.path().join("first.json");
    let replay = root.path().join("replay.json");

    fixture::project_command(root.path(), &project, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    fixture::project_command(root.path(), &project, &replay)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(
        fs::read(first).expect("first"),
        fs::read(replay).expect("replay")
    );
    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    assert_eq!(ledger.lines().count(), 1);
}

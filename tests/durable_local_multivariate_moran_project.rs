#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[path = "support/local_multivariate_moran_fixture.rs"]
mod fixture;

#[test]
fn local_multivariate_moran_reopens_as_a_byte_exact_hit() {
    let root = tempfile::tempdir().expect("root");
    fixture::write_inputs(root.path());
    let project = root.path().join("project");
    let first = root.path().join("first.json");
    let replay = root.path().join("replay.json");

    let run = |out: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("marklab binary");
        command.args([
            "project",
            "local-multivariate-moran",
            "--project",
            project.to_str().expect("project path"),
        ]);
        command.args(fixture::arguments(root.path(), out));
        command
    };

    run(&first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    run(&replay)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(
        fs::read(first).expect("first"),
        fs::read(replay).expect("replay")
    );

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    assert_eq!(ledger.lines().count(), 1, "a hit must not append execution");
}

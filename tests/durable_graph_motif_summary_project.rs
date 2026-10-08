#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "graph-motif-triangle-summary",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn compact_graph_motif_replays_without_repeating_the_permutation_workflow() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("motif.json");
    let project = directory.path().join("project");
    let direct = directory.path().join("direct.json");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &input,
        br#"{"nodes":[{"id":"a","label":"cell","stratum":"all"},{"id":"b","label":"cell","stratum":"all"},{"id":"c","label":"vessel","stratum":"all"}],"edges":[{"source_id":"a","target_id":"b"},{"source_id":"a","target_id":"c"},{"source_id":"b","target_id":"c"}],"motif_id":"cell-cell-vessel-triangle","required_labels":["cell","cell","vessel"],"permutations":39,"seed":77,"maximum_triples":100}"#,
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "graph",
            "motif-triangle-summary",
            "--input",
            input.to_str().unwrap(),
            "--out",
            direct.to_str().unwrap(),
        ])
        .assert()
        .success();
    project_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&direct).unwrap(), fs::read(&first).unwrap());
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

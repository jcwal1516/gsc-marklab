#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab::NearestSpaceResultDocument;

fn write_fixture(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let cells = root.join("cells.csv");
    let window = root.join("window.geojson");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
2,2,0,nearest_cli,baseline,unmarked,true,true\n\
8,2,0,nearest_cli,baseline,unmarked,true,true\n\
2,8,0,nearest_cli,baseline,unmarked,true,true\n\
8,8,0,nearest_cli,baseline,unmarked,true,true\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
    )
    .expect("window");
    (cells, window)
}

fn command(project: Option<&Path>, cells: &Path, window: &Path, out: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    if let Some(project) = project {
        command.args([
            "project",
            "nearest-space",
            "--project",
            project.to_str().expect("project path"),
        ]);
    } else {
        command.arg("nearest-space");
    }
    command.args([
        "--cells",
        cells.to_str().expect("cells path"),
        "--mask",
        window.to_str().expect("window path"),
        "--out",
        out.to_str().expect("output path"),
        "--r-max-um",
        "1",
        "--r-steps",
        "2",
        "--probe-grid-x",
        "2",
        "--probe-grid-y",
        "2",
        "--simulations",
        "19",
        "--seed",
        seed,
        "--alpha",
        "0.05",
        "--j-denominator-epsilon",
        "1e-12",
        "--memory-budget-mib",
        "4",
        "--max-nearest-queries",
        "100000",
        "--max-csr-draws",
        "100000",
    ]);
    command
}

#[test]
fn nearest_space_cli_runs_once_and_durable_project_replays_without_a_second_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let (cells, window) = write_fixture(directory.path());
    let direct_out = directory.path().join("direct");
    let project = directory.path().join("project");
    let miss_out = directory.path().join("miss");
    let hit_out = directory.path().join("hit");

    command(None, &cells, &window, &direct_out, "20260827")
        .assert()
        .success();
    command(Some(&project), &cells, &window, &miss_out, "20260827")
        .assert()
        .success();
    command(Some(&project), &cells, &window, &hit_out, "20260827")
        .assert()
        .success();

    let direct = NearestSpaceResultDocument::from_json(
        &fs::read_to_string(direct_out.join("result.json")).expect("direct result"),
    )
    .expect("direct document");
    let miss = NearestSpaceResultDocument::from_json(
        &fs::read_to_string(miss_out.join("result.json")).expect("miss result"),
    )
    .expect("miss document");
    let hit = NearestSpaceResultDocument::from_json(
        &fs::read_to_string(hit_out.join("result.json")).expect("hit result"),
    )
    .expect("hit document");
    assert_eq!(direct.analysis(), miss.analysis());
    assert_eq!(miss.analysis(), hit.analysis());
    assert_eq!(
        miss.workflow()
            .expect("miss identity")
            .cache_status
            .as_str(),
        "miss"
    );
    assert_eq!(
        hit.workflow().expect("hit identity").cache_status.as_str(),
        "hit"
    );
    assert_eq!(
        miss.workflow().expect("miss identity").cache_key,
        hit.workflow().expect("hit identity").cache_key
    );
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .expect("ledger")
            .lines()
            .count(),
        1
    );

    let changed_out = directory.path().join("changed-seed");
    command(Some(&project), &cells, &window, &changed_out, "20260828")
        .assert()
        .success();
    let changed = NearestSpaceResultDocument::from_json(
        &fs::read_to_string(changed_out.join("result.json")).expect("changed result"),
    )
    .expect("changed document");
    assert_eq!(
        changed
            .workflow()
            .expect("changed identity")
            .cache_status
            .as_str(),
        "miss"
    );
    assert_ne!(
        changed.workflow().expect("changed identity").cache_key,
        hit.workflow().expect("hit identity").cache_key
    );
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .expect("ledger after changed seed")
            .lines()
            .count(),
        2
    );
    let report = fs::read_to_string(hit_out.join("report.md")).expect("report");
    assert!(report.contains("F, G, and J"));
    assert!(report.contains("whole location pattern"));
    assert!(report.contains("patient-level inference"));
}

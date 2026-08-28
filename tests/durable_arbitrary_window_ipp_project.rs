#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, events: &Path, quadrature: &Path, window: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "arbitrary-window-ipp-likelihood",
        "--project",
        project.to_str().unwrap(),
        "--events",
        events.to_str().unwrap(),
        "--quadrature",
        quadrature.to_str().unwrap(),
        "--window",
        window.to_str().unwrap(),
        "--intercept",
        "0.6931471805599453",
        "--coefficient",
        "0",
        "--maximum-events",
        "2",
        "--maximum-quadrature-nodes",
        "4",
        "--maximum-work",
        "6",
        "--out",
        out.to_str().unwrap(),
    ]);
    command
}

#[test]
fn arbitrary_window_ipp_replays_across_processes_and_invalidates_on_source_change() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    fs::write(
        &events,
        "event_id,x_um,y_um,covariate,offset\ne-1,0.25,0.25,0,0\ne-2,3.5,0.5,0,0\n",
    )
    .unwrap();
    fs::write(
        &quadrature,
        "node_id,x_um,y_um,weight_um2,covariate,offset\nq-1,0.25,0.25,1,0,0\nq-2,1.75,0.25,1,0,0\nq-3,0.25,1.75,1,0,0\nq-4,3.5,0.5,1,0,0\n",
    )
    .unwrap();
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[2,0],[2,2],[0,2],[0,0]],[[0.5,0.5],[0.5,1.5],[1.5,1.5],[1.5,0.5],[0.5,0.5]]],[[[3,0],[4,0],[4,1],[3,1],[3,0]]]]}"#,
    )
    .unwrap();
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    command(&project, &events, &quadrature, &window, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &events, &quadrature, &window, &second)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.arbitrary_window_ipp_likelihood");
    assert_eq!(result["window"]["component_count"], 2);
    assert_eq!(result["window"]["hole_count"], 1);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );

    fs::write(
        &events,
        "event_id,x_um,y_um,covariate,offset\ne-1,0.25,0.25,0,0\ne-2,3.5,0.5,0,0\n\n",
    )
    .unwrap();
    command(
        &project,
        &events,
        &quadrature,
        &window,
        &directory.path().join("changed.json"),
    )
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=miss"));
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        2
    );
}

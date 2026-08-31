#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

fn project_command(
    project: &Path,
    events: &Path,
    membership: &Path,
    quadrature: &Path,
    window: &Path,
    output: &Path,
) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "arbitrary-window-ipp-spatial-ppc",
        "--project",
        project.to_str().unwrap(),
        "--events",
        events.to_str().unwrap(),
        "--event-membership",
        membership.to_str().unwrap(),
        "--quadrature",
        quadrature.to_str().unwrap(),
        "--window",
        window.to_str().unwrap(),
        "--intercept-prior-mean",
        "0",
        "--intercept-prior-sd",
        "3",
        "--coefficient-prior-mean",
        "0",
        "--coefficient-prior-sd",
        "2",
        "--chains",
        "2",
        "--tune",
        "100",
        "--draws",
        "100",
        "--target-accept",
        "0.9",
        "--seed",
        "20260828",
        "--prediction-seed",
        "20260829",
        "--neighbor-radius-um",
        "0.6",
        "--maximum-events",
        "50",
        "--maximum-quadrature-nodes",
        "4",
        "--maximum-neighbor-pairs",
        "4",
        "--maximum-draw-node-work",
        "800",
        "--timeout-seconds",
        "180",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn physical_spatial_ppc_replays_without_a_second_pymc_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let membership = directory.path().join("membership.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    let mut membership_csv = String::from("event_id,quadrature_node_id\n");
    for index in 0..10 {
        writeln!(event_csv, "left-{index:02},0.25,0.5,-1,0").unwrap();
        writeln!(membership_csv, "left-{index:02},left-a").unwrap();
    }
    for index in 0..40 {
        writeln!(event_csv, "right-{index:02},3.25,0.5,1,0").unwrap();
        writeln!(membership_csv, "right-{index:02},right-a").unwrap();
    }
    fs::write(&events, event_csv).unwrap();
    fs::write(&membership, membership_csv).unwrap();
    fs::write(
        &quadrature,
        "node_id,x_um,y_um,weight_um2,covariate,offset\nleft-a,0.25,0.5,0.5,-1,0\nleft-b,0.75,0.5,0.5,-1,0\nright-a,3.25,0.5,0.5,1,0\nright-b,3.75,0.5,0.5,1,0\n",
    )
    .unwrap();
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,1],[0,0]]],[[[3,0],[4,0],[4,1],[3,1],[3,0]]]]}"#,
    )
    .unwrap();

    project_command(&project, &events, &membership, &quadrature, &window, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(
        &project,
        &events,
        &membership,
        &quadrature,
        &window,
        &second,
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .success()
    .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.arbitrary_window_ipp_spatial_ppc");
    assert_eq!(result["observed_event_count"], 50);
    assert_eq!(result["neighbor_pair_count"], 2);
    assert_eq!(
        result["summaries"]["node_density_variance"]["observed"],
        1075.0
    );
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

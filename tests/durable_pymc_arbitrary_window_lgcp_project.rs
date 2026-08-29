#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

#[test]
fn arbitrary_window_lgcp_replays_without_a_second_backend_execution() {
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
    for (index, count) in [2, 4, 8, 16].into_iter().enumerate() {
        let ix = index % 2;
        let iy = index / 2;
        let covariate = ix as f64 * 2.0 - 1.0;
        for event in 0..count {
            let id = format!("c{index}-e{event:02}");
            writeln!(
                event_csv,
                "{id},{},{},{covariate},0",
                ix as f64 + 0.5,
                iy as f64 + 0.5
            )
            .unwrap();
            writeln!(membership_csv, "{id},q-{index}").unwrap();
        }
    }
    fs::write(&events, event_csv).unwrap();
    fs::write(&membership, membership_csv).unwrap();
    fs::write(
        &quadrature,
        "node_id,x_um,y_um,weight_um2,covariate,offset\nq-0,0.5,0.5,1,-1,0\nq-1,1.5,0.5,1,1,0\nq-2,0.5,1.5,1,-1,0\nq-3,1.5,1.5,1,1,0\n",
    )
    .unwrap();
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[2,0],[2,2],[0,2],[0,0]]]]}"#,
    )
    .unwrap();

    command(&project, &events, &membership, &quadrature, &window, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(
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
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_arbitrary_window_lgcp_fit"
    );
    assert_eq!(result["fit_state"], "complete");
}

fn command(
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
        "arbitrary-window-lgcp",
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
        "1",
        "--intercept-prior-sd",
        "1",
        "--coefficient-prior-mean",
        "0",
        "--coefficient-prior-sd",
        "1",
        "--field-amplitude",
        "0.2",
        "--field-length-scale-um",
        "1",
        "--jitter",
        "0.000001",
        "--chains",
        "2",
        "--tune",
        "1000",
        "--draws",
        "1000",
        "--target-accept",
        "0.95",
        "--seed",
        "16101",
        "--maximum-events",
        "30",
        "--maximum-quadrature-nodes",
        "4",
        "--maximum-draw-node-work",
        "8000",
        "--prediction-replicates",
        "8",
        "--prediction-seed",
        "16102",
        "--maximum-predictive-points",
        "10000",
        "--neighbor-radius-um",
        "1.1",
        "--maximum-neighbor-pairs",
        "4",
        "--timeout-seconds",
        "180",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

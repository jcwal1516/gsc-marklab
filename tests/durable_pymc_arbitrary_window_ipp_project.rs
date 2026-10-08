#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

fn command(
    project: &Path,
    events: &Path,
    quadrature: &Path,
    window: &Path,
    out: &Path,
    coefficient_prior_sd: &str,
) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "fit-arbitrary-window-ipp",
        "--project",
        project.to_str().unwrap(),
        "--events",
        events.to_str().unwrap(),
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
        coefficient_prior_sd,
        "--chains",
        "2",
        "--tune",
        "500",
        "--draws",
        "1000",
        "--target-accept",
        "0.9",
        "--seed",
        "20260828",
        "--maximum-events",
        "50",
        "--maximum-quadrature-nodes",
        "4",
        "--maximum-draw-node-work",
        "8000",
        "--timeout-seconds",
        "180",
        "--out",
        out.to_str().unwrap(),
    ]);
    command
}

#[test]
fn arbitrary_window_pymc_fit_replays_without_second_backend_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for index in 0..10 {
        writeln!(event_csv, "left-{index:02},0.25,0.25,-1,0").unwrap();
    }
    for index in 0..40 {
        writeln!(event_csv, "right-{index:02},3.5,0.5,1,0").unwrap();
    }
    fs::write(&events, event_csv).unwrap();
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
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    command(&project, &events, &quadrature, &window, &first, "2")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &events, &quadrature, &window, &second, "2")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_arbitrary_window_ipp_fit"
    );
    assert_eq!(result["backend"]["version"], "6.3.0");
    assert!(result["posterior"]["coefficient"]["mean"].as_f64().unwrap() > 0.5);

    command(
        &project,
        &events,
        &quadrature,
        &window,
        &directory.path().join("changed.json"),
        "3",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::ContentDigest;

fn write_fixture(events: &Path, grid: &Path) {
    let counts = [2, 5, 12];
    let mut events_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    for iy in 0..3 {
        for (ix, &count) in counts.iter().enumerate() {
            let covariate = ix as f64 - 1.0;
            for event in 0..count {
                let x = ix as f64 + (event as f64 + 1.0) / (count as f64 + 1.0);
                let y = iy as f64 + ((event * 7 % count) as f64 + 1.0) / (count as f64 + 1.0);
                writeln!(
                    events_csv,
                    "r{iy}c{ix}e{event},{x:.17},{y:.17},{covariate},0"
                )
                .expect("event row");
            }
        }
    }
    fs::write(events, events_csv).expect("events");
    let mut grid_csv = String::from("ix,iy,covariate,offset\n");
    for iy in 0..3 {
        for ix in 0..3 {
            writeln!(grid_csv, "{ix},{iy},{},0", ix as f64 - 1.0).expect("grid row");
        }
    }
    fs::write(grid, grid_csv).expect("grid");
}

fn project_command(
    project: &Path,
    events: &Path,
    grid: &Path,
    output: &Path,
    seed: &str,
) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "gridded-lgcp",
        "--project",
        project.to_str().expect("project path"),
        "--events",
        events.to_str().expect("events path"),
        "--grid",
        grid.to_str().expect("grid path"),
        "--xmin-um",
        "0",
        "--ymin-um",
        "0",
        "--xmax-um",
        "3",
        "--ymax-um",
        "3",
        "--grid-x",
        "3",
        "--grid-y",
        "3",
        "--intercept-prior-mean",
        "1.5",
        "--intercept-prior-sd",
        "1",
        "--coefficient-prior-mean",
        "0",
        "--coefficient-prior-sd",
        "1",
        "--field-amplitude",
        "0.4",
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
        seed,
        "--timeout-seconds",
        "180",
        "--out",
        output.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn gridded_lgcp_runs_once_then_replays_field_fit_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let grid = directory.path().join("grid.csv");
    write_fixture(&events, &grid);
    let events_digest = ContentDigest::from_bytes(&fs::read(&events).expect("events bytes"));
    let grid_digest = ContentDigest::from_bytes(&fs::read(&grid).expect("grid bytes"));
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    project_command(&project, &events, &grid, &first, "16101")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &events, &grid, &second, "16101")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let first_bytes = fs::read(&first).expect("first result");
    assert_eq!(first_bytes, fs::read(&second).expect("second result"));
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).expect("result JSON");
    assert_eq!(result["format"], "marklab.bayesian_gridded_lgcp_fit");
    assert_eq!(result["fit_state"], "complete", "{}", result["diagnostics"]);
    assert_eq!(result["backend"]["name"], "pymc");
    assert_eq!(
        result["model"]["construction"]["family"],
        "gridded_log_gaussian_cox_process"
    );
    assert_eq!(result["observed_event_count"], 57);
    assert_eq!(result["cells"].as_array().expect("cells").len(), 9);
    assert_eq!(result["diagnostics"]["divergences"], 0);

    project_command(
        &project,
        &events,
        &grid,
        &directory.path().join("changed-seed.json"),
        "16102",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));
    fs::write(
        &grid,
        format!("{}\n", fs::read_to_string(&grid).expect("grid text")),
    )
    .expect("changed grid bytes");
    project_command(
        &project,
        &events,
        &grid,
        &directory.path().join("changed-grid.json"),
        "16101",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(
        records.len(),
        1,
        "a replay and failed misses must not append"
    );
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    assert_eq!(record["identity"]["node"]["id"], "pymc-gridded-lgcp");
    assert_eq!(
        record["identity"]["inputs"][0]["digest"],
        events_digest.to_string()
    );
    assert_eq!(
        record["identity"]["inputs"][1]["digest"],
        grid_digest.to_string()
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_gridded_lgcp_worker_result"
    );
}

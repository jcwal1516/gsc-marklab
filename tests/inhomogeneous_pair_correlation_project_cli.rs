#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn inhomogeneous_g_csv_runs_durably_across_fresh_cli_processes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id\n\
slide-1:000000000,3400,300,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000001,3420,300,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000002,3440,300,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000003,3460,300,0,case-1,baseline,unmarked,true,true,slide-1\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[3609.4431999999997,488.1952],[3609.4431999999997,232.09279999999998],[3353.3408,232.09279999999998],[3353.3408,488.1952],[3609.4431999999997,488.1952]]]]}"#,
    )
    .expect("window");

    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "inhomogeneous-pair-correlation",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            window.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
            "--radii-um",
            "20",
            "--intensity-bandwidth-um",
            "20",
            "--pair-bandwidth-um",
            "5",
            "--grid-x",
            "16",
            "--grid-y",
            "16",
            "--simulations",
            "19",
            "--seed",
            "20260829",
            "--alpha",
            "0.05",
            "--minimum-intensity-per-um2",
            "1e-12",
            "--memory-budget-mib",
            "16",
            "--max-probes",
            "1000",
            "--max-intensity-evaluations",
            "1000000",
            "--max-pair-visits",
            "1000000",
            "--max-null-draws",
            "1000000",
        ]);
        command
    };
    run(&first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    let mut replay = run(&second);
    replay.env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1");
    replay
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(first).unwrap()).expect("result JSON");
    assert_eq!(result["case_id"], "case-1");
    assert_eq!(result["kernel"], "epanechnikov");
    assert_eq!(result["curve"].as_array().expect("curve").len(), 1);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

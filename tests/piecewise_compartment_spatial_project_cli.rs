#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn piecewise_compartment_kl_runs_durably_across_fresh_cli_processes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let observation = directory.path().join("observation.geojson");
    let negative = directory.path().join("stroma.geojson");
    let positive = directory.path().join("tumor.geojson");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id\n\
slide-1:000000000,2,5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000001,4,5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000002,6,5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000003,8,5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000004,9,5,0,case-1,baseline,unmarked,true,true,slide-1\n",
    )
    .expect("cells");
    fs::write(
        &observation,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[10,0],[10,10],[5,10],[0,10],[0,0]]]]}"#,
    )
    .expect("observation");
    fs::write(
        &negative,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[5,10],[0,10],[0,0]]]]}"#,
    )
    .expect("negative");
    fs::write(
        &positive,
        r#"{"type":"MultiPolygon","coordinates":[[[[5,0],[10,0],[10,10],[5,10],[5,0]]]]}"#,
    )
    .expect("positive");

    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "piecewise-compartment-spatial",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            cells.to_str().unwrap(),
            "--observation-mask",
            observation.to_str().unwrap(),
            "--negative-mask",
            negative.to_str().unwrap(),
            "--positive-mask",
            positive.to_str().unwrap(),
            "--negative-compartment-id",
            "stroma",
            "--positive-compartment-id",
            "tumor",
            "--out",
            output.to_str().unwrap(),
            "--radii-um",
            "1.1,2.1",
            "--simulations",
            "19",
            "--seed",
            "20260829",
            "--alpha",
            "0.05",
            "--memory-budget-mib",
            "16",
            "--max-boundary-segments",
            "64",
            "--max-compartment-queries",
            "16",
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
    assert_eq!(
        result["intensity"]["estimator"],
        "piecewise_constant_binary_compartment"
    );
    assert_eq!(result["intensity"]["negative"]["event_count"], 2);
    assert_eq!(result["intensity"]["positive"]["event_count"], 3);
    assert_eq!(result["curve"].as_array().expect("curve").len(), 2);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

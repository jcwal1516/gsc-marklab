#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn categorical_cross_g_csv_runs_durably_across_fresh_cli_processes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,histologic_compartment\n\
slide-1:000000000,0,0,1,case-1,baseline,cellvit,true,true,slide-1,Neoplastic\n\
slide-1:000000001,1,0,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory\n\
slide-1:000000002,3,0,0,case-1,baseline,cellvit,true,true,slide-1,Connective\n\
slide-1:000000003,10,0,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[11,-1],[11,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("window");

    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "categorical-cross-pair-correlation",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            window.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
            "--source-level",
            "Neoplastic",
            "--target-level",
            "Inflammatory",
            "--radii-um",
            "1.5,4.0",
            "--bandwidth-um",
            "0.5",
            "--permutations",
            "19",
            "--seed",
            "20260829",
            "--alpha",
            "0.05",
            "--memory-budget-mib",
            "16",
            "--max-pair-visits",
            "64",
            "--max-null-pair-evaluations",
            "1280",
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
    assert_eq!(result["source_level"], "Neoplastic");
    assert_eq!(result["target_level"], "Inflammatory");
    assert_eq!(result["kernel"], "epanechnikov");
    assert_eq!(result["curve"].as_array().expect("curve").len(), 2);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

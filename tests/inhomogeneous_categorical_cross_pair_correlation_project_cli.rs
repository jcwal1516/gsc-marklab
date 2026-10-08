#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn inhomogeneous_categorical_cross_g_replays_without_backend_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,histologic_compartment\n\
slide-1:000000000,9,10,1,case-1,baseline,cellvit,true,true,slide-1,Neoplastic\n\
slide-1:000000001,10,10,0,case-1,baseline,cellvit,true,true,slide-1,Neoplastic\n\
slide-1:000000002,14,10,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory\n\
slide-1:000000003,15,10,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[20,0],[20,20],[0,20],[0,0]]]]}"#,
    )
    .expect("window");

    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "inhomogeneous-categorical-cross-pair-correlation",
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
            "5",
            "--intensity-bandwidth-um",
            "2",
            "--pair-bandwidth-um",
            "1",
            "--grid-x",
            "20",
            "--grid-y",
            "20",
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
            "400",
            "--max-intensity-evaluations",
            "1000000",
            "--max-pair-visits",
            "10000",
            "--max-null-draws",
            "10000",
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
    assert_eq!(result["source_count"], 2);
    assert_eq!(result["target_count"], 2);
    assert_eq!(
        result["inference"]["null_model"],
        "independent_fixed_gridded_type_specific_inhomogeneous_binomial"
    );
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

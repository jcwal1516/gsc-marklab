#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn selected_gaussian_kl_runs_durably_across_fresh_cli_processes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id\n\
slide-1:000000000,2,2,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000001,3,2,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000002,7,7,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000003,9,7,0,case-1,baseline,unmarked,true,true,slide-1\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
    )
    .expect("window");
    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "gaussian-bandwidth-selected-spatial",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            window.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
            "--radii-um",
            "1.1",
            "--candidate-bandwidths-um",
            "1,2,3",
            "--grid-x",
            "10",
            "--grid-y",
            "10",
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
            "--max-bandwidth-candidates",
            "8",
            "--max-selection-intensity-evaluations",
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
    let result: marklab::SelectedInhomogeneousSpatialResult =
        marklab::exact_float_json::decode(&fs::read(first).unwrap()).expect("typed result");
    assert_eq!(result.selection.selected_bandwidth_um, 1.0);
    assert_eq!(result.analysis.intensity.bandwidth_um, 1.0);
    assert!(!result.selection.selection_uses_spatial_curve);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

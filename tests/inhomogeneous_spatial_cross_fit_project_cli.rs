#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn cross_fitted_intensity_replays_with_exact_fold_identity() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let miss = directory.path().join("miss.json");
    let hit = directory.path().join("hit.json");
    fs::write(
        &cells,
        concat!(
            "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id\n",
            "slide:a,2,2,0,case,baseline,unmarked,true,true,slide\n",
            "slide:b,3,2,0,case,baseline,unmarked,true,true,slide\n",
            "slide:c,7,7,0,case,baseline,unmarked,true,true,slide\n",
            "slide:d,9,7,0,case,baseline,unmarked,true,true,slide\n",
        ),
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
    )
    .expect("window");

    let run = |output: &std::path::Path, disabled: bool| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "inhomogeneous-spatial",
            "--project",
            project.to_str().expect("project path"),
            "--cells",
            cells.to_str().expect("cells path"),
            "--mask",
            window.to_str().expect("window path"),
            "--out",
            output.to_str().expect("output path"),
            "--radii-um",
            "1.1",
            "--bandwidth-um",
            "2",
            "--cross-fit-folds",
            "2",
            "--grid-x",
            "10",
            "--grid-y",
            "10",
            "--simulations",
            "19",
            "--seed",
            "20260831",
            "--alpha",
            "0.05",
            "--minimum-intensity-per-um2",
            "1e-12",
            "--memory-budget-mib",
            "16",
            "--max-probes",
            "1000",
            "--max-intensity-evaluations",
            "2000000",
            "--max-pair-visits",
            "1000000",
            "--max-null-draws",
            "1000000",
        ]);
        if disabled {
            command.env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1");
        }
        command
    };
    run(&miss, false)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    run(&hit, true)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&miss).expect("miss"), fs::read(&hit).expect("hit"));
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(miss).expect("result")).expect("result JSON");
    assert_eq!(
        result["intensity"]["cross_fit"],
        "balanced_cell_id_rank_2_fold"
    );
    assert!(result["intensity"]["point_values"]
        .as_array()
        .expect("point values")
        .iter()
        .all(|point| point["training_point_count"] == 2));
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .expect("ledger")
            .lines()
            .count(),
        1
    );
}

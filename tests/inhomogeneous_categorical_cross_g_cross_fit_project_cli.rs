#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn type_specific_cross_g_reuses_balanced_cell_id_cross_fitting() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let miss = directory.path().join("miss.json");
    let hit = directory.path().join("hit.json");
    fs::write(
        &cells,
        concat!(
            "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,histologic_compartment\n",
            "slide:001,5,5,0,case,baseline,cellvit,true,true,slide,Neoplastic\n",
            "slide:002,6,5,0,case,baseline,cellvit,true,true,slide,Neoplastic\n",
            "slide:003,5,6,0,case,baseline,cellvit,true,true,slide,Neoplastic\n",
            "slide:004,6,6,0,case,baseline,cellvit,true,true,slide,Neoplastic\n",
            "slide:005,12,12,0,case,baseline,cellvit,true,true,slide,Inflammatory\n",
            "slide:006,13,12,0,case,baseline,cellvit,true,true,slide,Inflammatory\n",
            "slide:007,12,13,0,case,baseline,cellvit,true,true,slide,Inflammatory\n",
            "slide:008,13,13,0,case,baseline,cellvit,true,true,slide,Inflammatory\n",
        ),
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[20,0],[20,20],[0,20],[0,0]]]]}"#,
    )
    .expect("window");
    let run = |output: &std::path::Path, disabled: bool| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "inhomogeneous-categorical-cross-pair-correlation",
            "--project",
            project.to_str().expect("project path"),
            "--cells",
            cells.to_str().expect("cells path"),
            "--mask",
            window.to_str().expect("window path"),
            "--out",
            output.to_str().expect("output path"),
            "--source-level",
            "Neoplastic",
            "--target-level",
            "Inflammatory",
            "--radii-um",
            "8",
            "--intensity-bandwidth-um",
            "3",
            "--cross-fit-folds",
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
            "20260831",
            "--alpha",
            "0.05",
            "--minimum-intensity-per-um2",
            "1e-12",
            "--memory-budget-mib",
            "16",
            "--max-probes",
            "400",
            "--max-intensity-evaluations",
            "5000000",
            "--max-pair-visits",
            "100000",
            "--max-null-draws",
            "10000",
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
        result["source_intensity"]["cross_fit"],
        "balanced_cell_id_rank_2_fold"
    );
    assert_eq!(
        result["target_intensity"]["cross_fit"],
        "balanced_cell_id_rank_2_fold"
    );
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .expect("ledger")
            .lines()
            .count(),
        1
    );
}

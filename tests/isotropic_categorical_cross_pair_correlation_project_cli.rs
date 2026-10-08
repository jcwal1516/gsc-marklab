#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::fs;

#[test]
fn isotropic_categorical_cross_g_replays_without_execution() {
    let dir = tempfile::tempdir().expect("dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("window.geojson");
    let project = dir.path().join("project");
    let first = dir.path().join("first.json");
    let second = dir.path().join("second.json");
    fs::write(&cells, "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,histologic_compartment\n\
s:0,0,0,0,c,baseline,cellvit,true,true,s,Tumor\n\
s:1,1,0,0,c,baseline,cellvit,true,true,s,Tumor\n\
s:2,2,0,0,c,baseline,cellvit,true,true,s,Stroma\n\
s:3,3,0,0,c,baseline,cellvit,true,true,s,Stroma\n").expect("cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-2,-2],[5,-2],[5,2],[-2,2],[-2,-2]]]]}"#,
    )
    .expect("mask");
    let run = |out: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "isotropic-categorical-cross-pair-correlation",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            mask.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--source-level",
            "Tumor",
            "--target-level",
            "Stroma",
            "--radii-um",
            "1",
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
            "--max-visible-arc-evaluations",
            "128",
            "--max-arc-segment-tests",
            "512",
            "--max-arc-membership-queries",
            "1024",
        ]);
        command
    };
    run(&first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    let mut hit = run(&second);
    hit.env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1");
    hit.assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).expect("JSON");
    assert_eq!(value["edge_correction"], "isotropic");
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

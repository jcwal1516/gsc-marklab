#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::fs;

#[test]
fn isotropic_g_fresh_process_miss_then_backend_disabled_hit() {
    let dir = tempfile::tempdir().expect("dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("window.geojson");
    let project = dir.path().join("project");
    let first = dir.path().join("first.json");
    let second = dir.path().join("second.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id\n\
s:0,1,5,0,c,baseline,unmarked,true,true,s\n\
s:1,3,5,0,c,baseline,unmarked,true,true,s\n",
    )
    .expect("cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
    )
    .expect("mask");
    let run = |out: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "isotropic-pair-correlation",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            mask.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--radii-um",
            "2",
            "--bandwidth-um",
            "0.5",
            "--simulations",
            "19",
            "--seed",
            "20260829",
            "--alpha",
            "0.05",
            "--memory-budget-mib",
            "16",
            "--max-pair-visits",
            "1000",
            "--max-visible-arc-evaluations",
            "2000",
            "--max-arc-segment-tests",
            "200000",
            "--max-arc-membership-queries",
            "100000",
            "--max-csr-draws",
            "100000",
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
    let value: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).expect("JSON");
    assert_eq!(value["format"], "marklab.isotropic_pair_correlation");
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

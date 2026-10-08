#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn isotropic_kl_runs_as_a_fresh_process_miss_then_backend_disabled_hit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id\n\
slide-1:000000000,1,5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000001,3,5,0,case-1,baseline,unmarked,true,true,slide-1\n",
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
            "isotropic-spatial",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            window.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
            "--radii-um",
            "2",
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
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(first).unwrap()).expect("result JSON");
    assert_eq!(result["format"], "marklab.isotropic_spatial");
    assert_eq!(result["analysis"]["correction"], "isotropic");
    assert_eq!(result["analysis"]["curve"][0]["directed_pairs"], 2);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

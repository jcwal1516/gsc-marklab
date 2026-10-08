#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn translation_g_runs_as_a_fresh_process_miss_then_backend_disabled_hit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id\n\
slide-1:000000000,0.5,0.5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000001,1.5,0.5,0,case-1,baseline,unmarked,true,true,slide-1\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[4,0],[4,1],[1,1],[1,4],[0,4],[0,0]]]]}"#,
    )
    .expect("window");

    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "translation-pair-correlation",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            window.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
            "--radii-um",
            "1",
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
            "--max-overlap-evaluations",
            "1000",
            "--max-overlap-candidate-work",
            "100000",
            "--max-overlap-output-vertices",
            "1000",
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
    assert_eq!(result["format"], "marklab.translation_pair_correlation");
    assert_eq!(result["analysis"]["correction"], "translation");
    assert_eq!(
        result["analysis"]["curve"][0]["ordered_pairs_in_support"],
        2
    );
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

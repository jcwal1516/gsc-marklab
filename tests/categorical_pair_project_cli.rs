#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn categorical_pair_csv_runs_durably_across_fresh_cli_processes() {
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
            "categorical-pair",
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
    assert_eq!(result["format"], "marklab.categorical-pair/1");
    assert_eq!(result["source_level"], "Neoplastic");
    assert_eq!(result["target_level"], "Inflammatory");
    assert_eq!(result["curve"].as_array().expect("curve").len(), 2);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn categorical_pair_reopens_high_precision_window_summary_without_bitwise_float_loss() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("result.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,histologic_compartment\n\
slide-1:000000000,3400,300,1,case-1,baseline,cellvit,true,true,slide-1,Neoplastic\n\
slide-1:000000001,3420,300,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory\n\
slide-1:000000002,3440,300,0,case-1,baseline,cellvit,true,true,slide-1,Connective\n\
slide-1:000000003,3460,300,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"coordinates":[[[[3609.4431999999997,488.1952],[3609.4431999999997,232.09279999999998],[3353.3408,232.09279999999998],[3353.3408,488.1952],[3609.4431999999997,488.1952]]],[[[1928.7712,712.2848],[1672.6688,712.2848],[1672.6688,968.3872],[1928.7712,968.3872],[1928.7712,712.2848]]],[[[2649.0591999999997,952.3807999999999],[2392.9568,952.3807999999999],[2392.9568,1208.4832],[2649.0591999999997,1208.4832],[2649.0591999999997,952.3807999999999]]],[[[3369.3471999999997,1208.4832],[3369.3471999999997,952.3807999999999],[3113.2448,952.3807999999999],[3113.2448,1208.4832],[3369.3471999999997,1208.4832]]],[[[1448.5792,1192.4768],[1192.4768,1192.4768],[1192.4768,1448.5792],[1448.5792,1448.5792],[1448.5792,1192.4768]]],[[[2168.8671999999997,1192.4768],[1912.7648,1192.4768],[1912.7648,1448.5792],[2168.8671999999997,1448.5792],[2168.8671999999997,1192.4768]]],[[[3833.5328,472.18879999999996],[3833.5328,712.2848],[3593.4368,712.2848],[3593.4368,968.3872],[3849.5391999999997,968.3872],[3849.5391999999997,728.2912],[4089.6351999999997,728.2912],[4089.6351999999997,472.18879999999996],[3833.5328,472.18879999999996]]],[[[4569.8272,472.18879999999996],[4313.7248,472.18879999999996],[4313.7248,728.2912],[4569.8272,728.2912],[4569.8272,472.18879999999996]]],[[[4569.8272,1208.4832],[4569.8272,952.3807999999999],[4313.7248,952.3807999999999],[4313.7248,1208.4832],[4569.8272,1208.4832]]],[[[5290.1152,952.3807999999999],[5034.0127999999995,952.3807999999999],[5034.0127999999995,1208.4832],[5290.1152,1208.4832],[5290.1152,952.3807999999999]]],[[[4089.6351999999997,1192.4768],[3833.5328,1192.4768],[3833.5328,1448.5792],[4089.6351999999997,1448.5792],[4089.6351999999997,1192.4768]]]],"type":"MultiPolygon"}"#,
    )
    .expect("window");

    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "categorical-pair",
        "--project",
        directory.path().join("project").to_str().unwrap(),
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
        "20,50",
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
    command.assert().success();
}

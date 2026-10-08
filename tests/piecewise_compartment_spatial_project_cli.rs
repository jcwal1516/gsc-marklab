#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

struct Inputs {
    cells: std::path::PathBuf,
    observation: std::path::PathBuf,
    negative: std::path::PathBuf,
    positive: std::path::PathBuf,
}

fn write_inputs(root: &std::path::Path) -> Inputs {
    let inputs = Inputs {
        cells: root.join("cells.csv"),
        observation: root.join("observation.geojson"),
        negative: root.join("stroma.geojson"),
        positive: root.join("tumor.geojson"),
    };
    fs::write(
        &inputs.cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id\n\
slide-1:000000000,2,5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000001,4,5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000002,6,5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000003,8,5,0,case-1,baseline,unmarked,true,true,slide-1\n\
slide-1:000000004,9,5,0,case-1,baseline,unmarked,true,true,slide-1\n",
    )
    .expect("cells");
    fs::write(
        &inputs.observation,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[10,0],[10,10],[5,10],[0,10],[0,0]]]]}"#,
    )
    .expect("observation");
    fs::write(
        &inputs.negative,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[5,10],[0,10],[0,0]]]]}"#,
    )
    .expect("negative");
    fs::write(
        &inputs.positive,
        r#"{"type":"MultiPolygon","coordinates":[[[[5,0],[10,0],[10,10],[5,10],[5,0]]]]}"#,
    )
    .expect("positive");
    inputs
}

#[test]
fn piecewise_compartment_kl_runs_durably_across_fresh_cli_processes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let inputs = write_inputs(directory.path());
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "piecewise-compartment-spatial",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            inputs.cells.to_str().unwrap(),
            "--observation-mask",
            inputs.observation.to_str().unwrap(),
            "--negative-mask",
            inputs.negative.to_str().unwrap(),
            "--positive-mask",
            inputs.positive.to_str().unwrap(),
            "--negative-compartment-id",
            "stroma",
            "--positive-compartment-id",
            "tumor",
            "--out",
            output.to_str().unwrap(),
            "--radii-um",
            "1.1,2.1",
            "--simulations",
            "19",
            "--seed",
            "20260829",
            "--alpha",
            "0.05",
            "--memory-budget-mib",
            "16",
            "--max-boundary-segments",
            "64",
            "--max-compartment-queries",
            "16",
            "--max-pair-visits",
            "1000000",
            "--max-null-draws",
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
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(first).unwrap()).expect("result JSON");
    assert_eq!(result["case_id"], "case-1");
    assert_eq!(
        result["intensity"]["estimator"],
        "piecewise_constant_binary_compartment"
    );
    assert_eq!(result["intensity"]["negative"]["event_count"], 2);
    assert_eq!(result["intensity"]["positive"]["event_count"], 3);
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
fn piecewise_compartment_g_runs_durably_across_fresh_cli_processes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let inputs = write_inputs(directory.path());
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "piecewise-compartment-pair-correlation",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            inputs.cells.to_str().unwrap(),
            "--observation-mask",
            inputs.observation.to_str().unwrap(),
            "--negative-mask",
            inputs.negative.to_str().unwrap(),
            "--positive-mask",
            inputs.positive.to_str().unwrap(),
            "--negative-compartment-id",
            "stroma",
            "--positive-compartment-id",
            "tumor",
            "--out",
            output.to_str().unwrap(),
            "--radii-um",
            "2",
            "--pair-bandwidth-um",
            "0.5",
            "--simulations",
            "19",
            "--seed",
            "20260829",
            "--alpha",
            "0.05",
            "--memory-budget-mib",
            "16",
            "--max-boundary-segments",
            "64",
            "--max-compartment-queries",
            "16",
            "--max-pair-visits",
            "1000000",
            "--max-null-draws",
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
    let result: marklab::PiecewiseCompartmentPairCorrelationResult =
        marklab::exact_float_json::decode(&fs::read(first).unwrap()).expect("typed result");
    assert_eq!(
        result.intensity.estimator,
        "piecewise_constant_binary_compartment"
    );
    assert_eq!(result.kernel, marklab::PairCorrelationKernel::Epanechnikov);
    assert_eq!(result.pair_bandwidth_um, 0.5);
    assert_eq!(result.curve.len(), 1);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

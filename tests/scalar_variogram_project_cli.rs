#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn nucleus_area_scalar_variogram_replays_across_fresh_processes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,histologic_compartment,nucleus_area_um2\n\
slide-1:000000000,0,0,1,case-1,baseline,cellvit,true,true,slide-1,Neoplastic,1\n\
slide-1:000000001,1,0,0,case-1,baseline,cellvit,true,true,slide-1,Neoplastic,2\n\
slide-1:000000002,2,0,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory,8\n\
slide-1:000000003,3,0,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory,9\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("window");

    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "scalar-variogram",
            "--project",
            project.to_str().unwrap(),
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            window.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
            "--lag-edges-um",
            "0,1.5,2.5,3.5",
            "--condition-by-histologic-compartment",
            "--permutations",
            "31",
            "--seed",
            "20260829",
            "--alpha",
            "0.25",
            "--memory-budget-mib",
            "16",
            "--max-pair-visits",
            "6",
            "--max-permutation-pair-evaluations",
            "186",
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
    assert_eq!(result["format"], "marklab.scalar-semivariogram-inference/1");
    assert_eq!(result["mark_id"], "nucleus_area_um2");
    assert_eq!(result["conditioning"], "histologic_compartment");
    assert_eq!(result["pair_visits"], 6);
    assert_eq!(result["observed_curve"][0]["pair_count"], 3);
    let semivariance = f64::from_bits(
        result["observed_curve"][0]["semivariance"]["__marklab_f64_bits"]
            .as_u64()
            .expect("semivariance bits"),
    );
    assert!((semivariance - 19.0 / 3.0).abs() < 1e-12);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

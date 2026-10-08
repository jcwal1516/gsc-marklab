#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[path = "support/local_multivariate_moran_fixture.rs"]
mod fixture;

#[test]
fn local_multivariate_moran_reopens_as_a_byte_exact_hit() {
    let root = tempfile::tempdir().expect("root");
    fixture::write_inputs(root.path());
    let project = root.path().join("project");
    let first = root.path().join("first.json");
    let replay = root.path().join("replay.json");

    let run = |out: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("marklab binary");
        command.args([
            "project",
            "local-multivariate-moran",
            "--project",
            project.to_str().expect("project path"),
        ]);
        command.args(fixture::arguments(root.path(), out));
        command
    };

    run(&first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    run(&replay)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(
        fs::read(first).expect("first"),
        fs::read(replay).expect("replay")
    );

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    assert_eq!(ledger.lines().count(), 1, "a hit must not append execution");
}

#[test]
fn direct_and_project_use_the_same_float_codec_for_decimal_cellvit_values() {
    let root = tempfile::tempdir().expect("root");
    let mut source =
        String::from("cell_id,x_um,y_um,cellvit_pc_000,cellvit_pc_001,cellvit_pc_002\n");
    for row in 0..100 {
        let first = (row * 17 % 101) as f64 / 37.0 - 1.0;
        let second = (row * 29 % 103) as f64 / 41.0 - 1.2;
        let third = (row * 43 % 107) as f64 / 53.0 - 0.8;
        source.push_str(&format!(
            "slide-decimal:p{row},{},{},{first:.17},{second:.17},{third:.17}\n",
            row % 10,
            row / 10
        ));
    }
    fs::write(root.path().join("points.csv"), source).expect("source");
    fs::write(
        root.path().join("window.geojson"),
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[11,-1],[11,11],[-1,11],[-1,-1]]]]}"#,
    )
    .expect("window");
    let direct = root.path().join("direct.json");
    let miss = root.path().join("miss.json");
    let project = root.path().join("project-decimal");

    let mut direct_command = Command::cargo_bin("marklab").expect("marklab binary");
    direct_command.args([
        "numerics",
        "local-multivariate-moran",
        "--input",
        root.path().join("points.csv").to_str().unwrap(),
        "--window",
        root.path().join("window.geojson").to_str().unwrap(),
        "--radius-um",
        "20",
        "--permutations",
        "7",
        "--seed",
        "88",
        "--maximum-points",
        "100",
        "--maximum-dimension",
        "3",
        "--maximum-directed-edges",
        "9900",
        "--maximum-permutation-edge-evaluations",
        "69300",
        "--memory-budget-mib",
        "8",
        "--out",
        direct.to_str().unwrap(),
    ]);
    direct_command.assert().success();
    let mut project_command = Command::cargo_bin("marklab").expect("marklab binary");
    project_command.args([
        "project",
        "local-multivariate-moran",
        "--project",
        project.to_str().unwrap(),
        "--input",
        root.path().join("points.csv").to_str().unwrap(),
        "--window",
        root.path().join("window.geojson").to_str().unwrap(),
        "--radius-um",
        "20",
        "--permutations",
        "7",
        "--seed",
        "88",
        "--maximum-points",
        "100",
        "--maximum-dimension",
        "3",
        "--maximum-directed-edges",
        "9900",
        "--maximum-permutation-edge-evaluations",
        "69300",
        "--memory-budget-mib",
        "8",
        "--out",
        miss.to_str().unwrap(),
    ]);
    project_command.assert().success();
    assert_eq!(fs::read(direct).unwrap(), fs::read(miss).unwrap());
}

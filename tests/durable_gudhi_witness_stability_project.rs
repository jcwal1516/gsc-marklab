#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "witness-persistence-stability",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn witness_stability_replays_without_more_gudhi_executions() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("stability.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let spec = serde_json::json!({
        "points":[
            {"id":"a","coordinates_um":[0.0]},
            {"id":"b","coordinates_um":[1.0]},
            {"id":"c","coordinates_um":[2.0]},
            {"id":"d","coordinates_um":[3.0]},
            {"id":"e","coordinates_um":[4.0]}
        ],
        "landmark_method":"farthest_point",
        "landmark_count":3,
        "maximum_dimension":1,
        "nu":0,
        "max_scale_um":1.0,
        "coefficient_field":2,
        "maximum_simplices":100,
        "timeout_seconds":30,
        "perturbation_replicates":2,
        "maximum_coordinate_jitter_um":0.1,
        "seed":20260829,
        "minimum_landmark_id_match_fraction":0.66,
        "maximum_coverage_radius_change_um":0.25,
        "maximum_simplex_count_l1_change":10,
        "maximum_total_persistence_change_um_squared":1.0,
        "maximum_backend_executions":3,
        "maximum_total_point_work":15,
        "maximum_total_simplex_budget":300,
        "maximum_total_timeout_seconds":90
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");

    command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.witness_persistence_stability");
    assert_eq!(result["backend_executions"], 3);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn dense_witness_stability_above_one_mib_replays_without_more_gudhi_executions() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("dense-stability.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let points = (0..512)
        .map(|index| {
            let column = index % 32;
            let row = index / 32;
            serde_json::json!({
                "id":format!("dense-field:{index:09}"),
                "coordinates_um":[
                    column as f64 * 7.0 + (row % 3) as f64 * 0.37,
                    row as f64 * 7.0 + (column % 5) as f64 * 0.23
                ]
            })
        })
        .collect::<Vec<_>>();
    let spec = serde_json::json!({
        "points":points,
        "landmark_method":"farthest_point",
        "landmark_count":64,
        "maximum_dimension":2,
        "nu":0,
        "max_scale_um":200.0,
        "coefficient_field":2,
        "maximum_simplices":500000,
        "timeout_seconds":180,
        "perturbation_replicates":1,
        "maximum_coordinate_jitter_um":1.0,
        "seed":20260830,
        "minimum_landmark_id_match_fraction":0.5,
        "maximum_coverage_radius_change_um":25.0,
        "maximum_simplex_count_l1_change":500000,
        "maximum_total_persistence_change_um_squared":1000000000.0,
        "maximum_backend_executions":2,
        "maximum_total_point_work":1024,
        "maximum_total_simplex_budget":1000000,
        "maximum_total_timeout_seconds":360
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");

    command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    assert!(fs::metadata(&first).unwrap().len() > 1024 * 1024);
    command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

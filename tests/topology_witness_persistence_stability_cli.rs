#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn witness_stability_has_an_exact_zero_jitter_oracle_and_total_work_bound() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("stability.json");
    let output = directory.path().join("result.json");
    let mut spec = serde_json::json!({
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
        "maximum_coordinate_jitter_um":0.0,
        "seed":20260829,
        "minimum_landmark_id_match_fraction":1.0,
        "maximum_coverage_radius_change_um":0.0,
        "maximum_simplex_count_l1_change":0,
        "maximum_total_persistence_change_um_squared":0.0,
        "maximum_backend_executions":3,
        "maximum_total_point_work":15,
        "maximum_total_simplex_budget":300,
        "maximum_total_timeout_seconds":90
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "witness-persistence-stability",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(&output).expect("result")).expect("result JSON");
    assert_eq!(result["format"], "marklab.witness_persistence_stability");
    assert_eq!(result["backend_executions"], 3);
    assert_eq!(result["perturbations"].as_array().unwrap().len(), 2);
    assert_eq!(result["stable_under_declared_thresholds"], true);
    assert!(result["perturbations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| {
            row["landmark_id_match_fraction"] == 1.0
                && row["coverage_radius_change_um"] == 0.0
                && row["simplex_count_l1_change"] == 0
                && row["maximum_total_persistence_change_um_squared"] == 0.0
        }));

    spec["maximum_total_point_work"] = serde_json::json!(14);
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("one-short fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "witness-persistence-stability",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "aggregate point work exceeds caller maximum",
        ));
}

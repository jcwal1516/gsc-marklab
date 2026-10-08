#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn request(maximum_comparisons: u64) -> serde_json::Value {
    serde_json::json!({
        "stability": {
            "points": [
                {"id":"a","coordinates_um":[0.0,0.0]},
                {"id":"b","coordinates_um":[2.0,0.0]},
                {"id":"c","coordinates_um":[0.0,2.0]},
                {"id":"d","coordinates_um":[2.0,2.0]},
                {"id":"e","coordinates_um":[1.0,1.0]}
            ],
            "landmark_method":"farthest_point",
            "landmark_count":3,
            "maximum_dimension":2,
            "nu":0,
            "max_scale_um":4.0,
            "coefficient_field":2,
            "maximum_simplices":1_000,
            "timeout_seconds":60,
            "perturbation_replicates":2,
            "maximum_coordinate_jitter_um":0.0,
            "seed":19,
            "minimum_landmark_id_match_fraction":1.0,
            "maximum_coverage_radius_change_um":0.0,
            "maximum_simplex_count_l1_change":0,
            "maximum_total_persistence_change_um_squared":0.0,
            "maximum_backend_executions":3,
            "maximum_total_point_work":15,
            "maximum_total_simplex_budget":3_000,
            "maximum_total_timeout_seconds":180
        },
        "maximum_bottleneck_distance_um_squared":0.0,
        "maximum_bottleneck_comparisons":maximum_comparisons,
        "maximum_bottleneck_interval_budget":12_000,
        "maximum_bottleneck_backend_executions":1,
        "bottleneck_timeout_seconds":60,
        "maximum_total_backend_executions":4
    })
}

#[test]
fn witness_bottleneck_stability_has_an_exact_zero_jitter_oracle_and_work_bound() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("bottleneck.json");
    let output = directory.path().join("result.json");
    fs::write(&input, serde_json::to_vec(&request(6)).unwrap()).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "witness-persistence-bottleneck-stability",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(&output).unwrap()).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.witness_persistence_bottleneck_stability"
    );
    assert_eq!(result["bottleneck_metric"], "gudhi_exact_linf");
    assert_eq!(result["bottleneck_comparisons"], 6);
    assert_eq!(result["maximum_finite_bottleneck_distance_um_squared"], 0.0);
    assert_eq!(result["has_infinite_essential_mismatch"], false);
    assert_eq!(result["stable_under_all_declared_thresholds"], true);
    assert!(result["perturbations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["by_dimension"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dimension| dimension["bottleneck_distance_um_squared"] == 0.0)));

    fs::write(&input, serde_json::to_vec(&request(5)).unwrap()).expect("one-short fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "witness-persistence-bottleneck-stability",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "bottleneck comparisons exceed caller maximum",
        ));
}

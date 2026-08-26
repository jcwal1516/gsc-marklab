#![cfg(feature = "cli")]

use std::fs;

#[test]
fn inhomogeneous_and_directed_cross_k_match_hand_oracles() {
    let directory = tempfile::tempdir().unwrap();
    let common = serde_json::json!({
        "window": {"minimum": [0.0, 0.0, 0.0], "maximum": [10.0, 10.0, 10.0]},
        "coordinate_unit": "micrometer",
        "voxel_spacing": [1.0, 1.0, 1.0],
        "anisotropy_matrix": null,
        "radii_um": [0.5, 1.0, 2.0],
        "correction": "none"
    });
    let inhomogeneous = merge(
        common.clone(),
        serde_json::json!({
            "points": [
                {"id": "a", "coordinates": [4.0, 5.0, 5.0], "intensity_per_um3": 0.002},
                {"id": "b", "coordinates": [5.0, 5.0, 5.0], "intensity_per_um3": 0.002}
            ],
            "maximum_unordered_pairs": 10
        }),
    );
    let cross = merge(
        common,
        serde_json::json!({
            "type_a_points": [
                {"id": "a", "coordinates": [4.0, 5.0, 5.0], "intensity_per_um3": 0.001}
            ],
            "type_b_points": [
                {"id": "b", "coordinates": [5.0, 5.0, 5.0], "intensity_per_um3": 0.001}
            ],
            "maximum_cross_pairs": 10
        }),
    );

    let in_path = directory.path().join("inhomogeneous.json");
    let cross_path = directory.path().join("cross.json");
    fs::write(&in_path, serde_json::to_vec(&inhomogeneous).unwrap()).unwrap();
    fs::write(&cross_path, serde_json::to_vec(&cross).unwrap()).unwrap();
    let in_out = directory.path().join("inhomogeneous-out.json");
    let cross_out = directory.path().join("cross-out.json");
    run("inhomogeneous-k", &in_path, &in_out);
    run("cross-k", &cross_path, &cross_out);

    let in_result: serde_json::Value = serde_json::from_slice(&fs::read(in_out).unwrap()).unwrap();
    assert_eq!(in_result["format"], "marklab.inhomogeneous_k3d");
    assert_close(in_result["curve"][1]["k_um3"].as_f64().unwrap(), 500.0);
    let cross_result: serde_json::Value =
        serde_json::from_slice(&fs::read(cross_out).unwrap()).unwrap();
    assert_eq!(cross_result["format"], "marklab.directed_cross_k3d");
    assert_close(
        cross_result["curve"][1]["cross_k_um3"].as_f64().unwrap(),
        1000.0,
    );
    assert!(cross_result["curve"][1]["cross_g"].as_f64().unwrap() > 0.0);
    assert_eq!(cross_result["direction"], "type_a_to_type_b");
}

fn merge(mut left: serde_json::Value, right: serde_json::Value) -> serde_json::Value {
    left.as_object_mut()
        .unwrap()
        .extend(right.as_object().unwrap().clone());
    left
}

fn run(command: &str, input: &std::path::Path, output: &std::path::Path) {
    assert_cmd::Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "spatial3d",
            command,
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-10,
        "{actual} != {expected}"
    );
}

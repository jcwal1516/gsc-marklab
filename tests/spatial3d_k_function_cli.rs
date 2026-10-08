#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn millimetre_cuboid_k_matches_three_dimensional_hand_oracles_and_replays() {
    let directory = tempfile::tempdir().unwrap();
    for (correction, expected) in [
        ("none", 1000.0),
        ("border", 1000.0),
        ("translation", 10000.0 / 9.0),
    ] {
        let input = directory.path().join(format!("{correction}.json"));
        fs::write(
            &input,
            serde_json::to_vec(&serde_json::json!({
                "points": [
                    {"id": "a", "coordinates": [0.004, 0.005, 0.005]},
                    {"id": "b", "coordinates": [0.005, 0.005, 0.005]}
                ],
                "window": {"minimum": [0.0, 0.0, 0.0], "maximum": [0.01, 0.01, 0.01]},
                "coordinate_unit": "millimeter",
                "voxel_spacing": [0.001, 0.001, 0.001],
                "anisotropy_matrix": null,
                "radii_um": [0.5, 1.0, 2.0],
                "correction": correction,
                "maximum_unordered_pairs": 10
            }))
            .unwrap(),
        )
        .unwrap();
        let first = directory.path().join(format!("{correction}-first.json"));
        let second = directory.path().join(format!("{correction}-second.json"));
        run(&input, &first);
        run(&input, &second);
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
        let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
        assert_eq!(result["format"], "marklab.homogeneous_k3d");
        assert_eq!(result["dimension"], 3);
        assert_eq!(result["metric"], "euclidean_physical_um");
        assert_eq!(result["window"]["volume_um3"], 1000.0);
        assert_eq!(
            result["normalized_points"][0]["coordinates_um"],
            serde_json::json!([4.0, 5.0, 5.0])
        );
        assert_close(
            result["curve"][1]["k_um3"].as_f64().unwrap(),
            expected,
            1.0e-10,
        );
        assert_eq!(result["unordered_pairs_visited"], 1);
    }
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "spatial3d",
            "k-function",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected}"
    );
}

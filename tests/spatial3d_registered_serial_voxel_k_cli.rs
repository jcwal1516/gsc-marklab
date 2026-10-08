#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn registered_serial_sections_preserve_a_missing_plane_and_volume_identity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("registered.json");
    let output = directory.path().join("result.json");
    fs::write(
        &input,
        serde_json::to_vec_pretty(&serde_json::json!({
            "patient_id": "patient-1",
            "specimen_id": "specimen-1",
            "timepoint_id": "timepoint-1",
            "anatomical_site": "colon-primary",
            "volume_frame_id": "registered-volume-1",
            "sections": [
                {
                    "section_id": "section-0", "ordinal": 0, "z_center_um": 0.5,
                    "thickness_um": 1.0, "status": "observed",
                    "source_frame_id": "section-0-xy",
                    "placement_coefficients": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    "uncertainty": null,
                    "points": [{"id": "a", "coordinates_um": [0.5, 0.5]}]
                },
                {
                    "section_id": "section-1", "ordinal": 1, "z_center_um": 1.5,
                    "thickness_um": 1.0, "status": "missing",
                    "source_frame_id": null, "placement_coefficients": null,
                    "uncertainty": null, "points": []
                },
                {
                    "section_id": "section-2", "ordinal": 2, "z_center_um": 2.5,
                    "thickness_um": 1.0, "status": "observed",
                    "source_frame_id": "section-2-xy",
                    "placement_coefficients": [1.0, 0.0, 1.0, 0.0, 1.0, 0.0],
                    "uncertainty": null,
                    "points": [{"id": "b", "coordinates_um": [0.5, 0.5]}]
                }
            ],
            "window": {
                "origin": [0.0, 0.0, 0.0], "voxel_size": [1.0, 1.0, 1.0],
                "dimensions": [2, 1, 3], "occupied_voxels": [[0, 0, 0], [1, 0, 2]],
                "maximum_grid_voxels": 6
            },
            "anisotropy_matrix": null,
            "radii_um": [3.0], "correction": "none",
            "maximum_unordered_pairs": 1,
            "maximum_boundary_face_checks": 100,
            "maximum_translation_voxel_pair_checks": 1,
            "memory_budget_mib": 16
        }))
        .unwrap(),
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "registered-serial-voxel-k",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.registered_serial_voxel_k3d");
    assert_eq!(result["patient_id"], "patient-1");
    assert_eq!(result["specimen_id"], "specimen-1");
    assert_eq!(result["timepoint_id"], "timepoint-1");
    assert_eq!(result["anatomical_site"], "colon-primary");
    assert_eq!(result["volume_frame_id"], "registered-volume-1");
    assert_eq!(result["section_count"], 3);
    assert_eq!(result["observed_section_count"], 2);
    assert_eq!(result["missing_section_count"], 1);
    assert_eq!(result["analysis"]["window"]["connected_components"], 2);
    assert_eq!(result["analysis"]["window"]["occupied_voxel_count"], 2);
    assert_eq!(
        result["analysis"]["normalized_points"][0]["coordinates_um"],
        serde_json::json!([0.5, 0.5, 0.5])
    );
    assert_eq!(
        result["analysis"]["normalized_points"][1]["coordinates_um"],
        serde_json::json!([1.5, 0.5, 2.5])
    );
    assert_eq!(result["analysis"]["curve"][0]["k_um3"], 2.0);
    assert_eq!(
        result["claim_status"],
        "registered_serial_geometry_descriptive_only"
    );

    let mut distorted: serde_json::Value =
        serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    distorted["sections"][0]["status"] = serde_json::json!("distorted");
    distorted["sections"][0]["uncertainty"] = serde_json::json!({
        "uncertainty_id": "registration-posterior-1",
        "conservative_radius_um": 0.25
    });
    let distorted_input = directory.path().join("distorted.json");
    fs::write(
        &distorted_input,
        serde_json::to_vec_pretty(&distorted).unwrap(),
    )
    .unwrap();
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "registered-serial-voxel-k",
            "--input",
            distorted_input.to_str().unwrap(),
            "--out",
            directory
                .path()
                .join("must-not-exist.json")
                .to_str()
                .unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "requires transform-draw propagation",
        ));
}

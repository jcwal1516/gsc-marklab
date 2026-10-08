#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn volume(specimen: &str, timepoint: &str, second_x: f64) -> serde_json::Value {
    serde_json::json!({
        "patient_id": "patient-1", "specimen_id": specimen, "timepoint_id": timepoint,
        "anatomical_site": "colon-primary", "volume_frame_id": format!("{timepoint}-volume"),
        "sections": [{
            "section_id": format!("{timepoint}-section"), "ordinal": 0,
            "z_center_um": 0.5, "thickness_um": 1.0, "status": "observed",
            "source_frame_id": format!("{timepoint}-xy"),
            "placement_coefficients": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            "uncertainty": null,
            "points": [
                {"id": format!("{timepoint}-a"), "coordinates_um": [0.5, 0.5]},
                {"id": format!("{timepoint}-b"), "coordinates_um": [second_x, 0.5]}
            ]
        }],
        "window": {"origin": [0.0, 0.0, 0.0], "voxel_size": [1.0, 1.0, 1.0],
            "dimensions": [3, 1, 1], "occupied_voxels": [[0, 0, 0], [1, 0, 0], [2, 0, 0]],
            "maximum_grid_voxels": 3},
        "anisotropy_matrix": null, "radii_um": [1.1], "correction": "none",
        "maximum_unordered_pairs": 1, "maximum_boundary_face_checks": 100,
        "maximum_translation_voxel_pair_checks": 1, "memory_budget_mib": 16
    })
}

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "spatial3d-registered-longitudinal-voxel-k-change",
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
fn paired_registered_longitudinal_change_replays_without_reanalysis() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let project = directory.path().join("project");
    let direct = directory.path().join("direct.json");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let deformation = (0..32)
        .map(|index| vec![if index % 2 == 0 { -0.25 } else { 0.25 }])
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "patient_id": "patient-1", "lesion_id": "lesion-primary-1",
        "anatomical_site": "colon-primary", "baseline_elapsed_days": 0.0,
        "follow_up_elapsed_days": 28.0,
        "treatment_interval": {"interval_id": "interval-1",
            "treatment_or_exposure_id": "neoadjuvant-regimen-a", "start_day": 1.0, "end_day": 21.0},
        "cross_time_registration_id": "longitudinal-registration-1",
        "deformation_posterior_id": "registration-sensitivity-1",
        "baseline": volume("specimen-baseline", "timepoint-baseline", 2.5),
        "follow_up": volume("specimen-follow-up", "timepoint-follow-up", 1.5),
        "deformation_only_delta_k_draws_um3": deformation,
        "negative_control_delta_k_um3": [0.0], "independent_change_delta_k_um3": [2.8],
        "maximum_deformation_draws": 64, "maximum_radius_draw_evaluations": 128,
        "memory_budget_mib": 16
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "registered-longitudinal-voxel-k-change",
            "--input",
            input.to_str().unwrap(),
            "--out",
            direct.to_str().unwrap(),
        ])
        .assert()
        .success();
    project_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let direct_value: serde_json::Value =
        serde_json::from_slice(&fs::read(&direct).unwrap()).unwrap();
    let durable_value: serde_json::Value =
        serde_json::from_slice(&fs::read(&first).unwrap()).unwrap();
    assert_eq!(direct_value, durable_value);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        durable_value["curve"][0]["change_supported_beyond_registration"],
        true
    );
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn registered_volume(
    patient_id: &str,
    specimen_id: &str,
    timepoint_id: &str,
    volume_frame_id: &str,
    second_x: f64,
) -> serde_json::Value {
    serde_json::json!({
        "patient_id": patient_id,
        "specimen_id": specimen_id,
        "timepoint_id": timepoint_id,
        "anatomical_site": "colon-primary",
        "volume_frame_id": volume_frame_id,
        "sections": [{
            "section_id": format!("{timepoint_id}-section-0"),
            "ordinal": 0,
            "z_center_um": 0.5,
            "thickness_um": 1.0,
            "status": "observed",
            "source_frame_id": format!("{timepoint_id}-xy"),
            "placement_coefficients": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            "uncertainty": null,
            "points": [
                {"id": format!("{timepoint_id}-a"), "coordinates_um": [0.5, 0.5]},
                {"id": format!("{timepoint_id}-b"), "coordinates_um": [second_x, 0.5]}
            ]
        }],
        "window": {
            "origin": [0.0, 0.0, 0.0],
            "voxel_size": [1.0, 1.0, 1.0],
            "dimensions": [3, 1, 1],
            "occupied_voxels": [[0, 0, 0], [1, 0, 0], [2, 0, 0]],
            "maximum_grid_voxels": 3
        },
        "anisotropy_matrix": null,
        "radii_um": [1.1],
        "correction": "none",
        "maximum_unordered_pairs": 1,
        "maximum_boundary_face_checks": 100,
        "maximum_translation_voxel_pair_checks": 1,
        "memory_budget_mib": 16
    })
}

fn fixture(patient_id: &str) -> serde_json::Value {
    let deformation_draws = (0..32)
        .map(|index| vec![if index % 2 == 0 { -0.25 } else { 0.25 }])
        .collect::<Vec<_>>();
    serde_json::json!({
        "patient_id": "patient-1",
        "lesion_id": "lesion-primary-1",
        "anatomical_site": "colon-primary",
        "baseline_elapsed_days": 0.0,
        "follow_up_elapsed_days": 28.0,
        "treatment_interval": {
            "interval_id": "interval-1",
            "treatment_or_exposure_id": "neoadjuvant-regimen-a",
            "start_day": 1.0,
            "end_day": 21.0
        },
        "cross_time_registration_id": "longitudinal-registration-1",
        "deformation_posterior_id": "registration-sensitivity-1",
        "baseline": registered_volume(
            patient_id,
            "specimen-baseline",
            "timepoint-baseline",
            "volume-baseline",
            2.5
        ),
        "follow_up": registered_volume(
            "patient-1",
            "specimen-follow-up",
            "timepoint-follow-up",
            "volume-follow-up",
            1.5
        ),
        "deformation_only_delta_k_draws_um3": deformation_draws,
        "negative_control_delta_k_um3": [0.0],
        "independent_change_delta_k_um3": [2.8],
        "maximum_deformation_draws": 64,
        "maximum_radius_draw_evaluations": 128,
        "memory_budget_mib": 16
    })
}

#[test]
fn paired_registered_volumes_separate_change_from_registration_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("longitudinal.json");
    let output = directory.path().join("result.json");
    fs::write(
        &input,
        serde_json::to_vec_pretty(&fixture("patient-1")).unwrap(),
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "registered-longitudinal-voxel-k-change",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.registered_longitudinal_voxel_k_change"
    );
    assert_eq!(result["patient_id"], "patient-1");
    assert_eq!(result["lesion_id"], "lesion-primary-1");
    assert_eq!(result["anatomical_site"], "colon-primary");
    assert_eq!(result["baseline_timepoint_id"], "timepoint-baseline");
    assert_eq!(result["follow_up_timepoint_id"], "timepoint-follow-up");
    assert_eq!(
        result["treatment_interval"]["treatment_or_exposure_id"],
        "neoadjuvant-regimen-a"
    );
    assert_eq!(result["curve"][0]["baseline_k_um3"], 0.0);
    assert_eq!(result["curve"][0]["follow_up_k_um3"], 3.0);
    assert_eq!(result["curve"][0]["observed_delta_k_um3"], 3.0);
    assert_eq!(
        result["curve"][0]["registration_adjusted_delta_mean_um3"],
        3.0
    );
    assert_eq!(
        result["curve"][0]["registration_adjusted_lower_95_um3"],
        2.75
    );
    assert_eq!(
        result["curve"][0]["registration_adjusted_upper_95_um3"],
        3.25
    );
    assert_eq!(
        result["curve"][0]["change_supported_beyond_registration"],
        true
    );
    assert_eq!(
        result["claim_status"],
        "paired_registered_change_diagnostic_not_causal_or_same_cell"
    );

    let mut null_change = fixture("patient-1");
    null_change["follow_up"]["sections"][0]["points"][1]["coordinates_um"] =
        serde_json::json!([2.5, 0.5]);
    null_change["independent_change_delta_k_um3"] = serde_json::json!([0.0]);
    let null_input = directory.path().join("null.json");
    let null_output = directory.path().join("null-result.json");
    fs::write(
        &null_input,
        serde_json::to_vec_pretty(&null_change).unwrap(),
    )
    .unwrap();
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "registered-longitudinal-voxel-k-change",
            "--input",
            null_input.to_str().unwrap(),
            "--out",
            null_output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let null_result: serde_json::Value =
        serde_json::from_slice(&fs::read(null_output).unwrap()).unwrap();
    assert_eq!(null_result["curve"][0]["observed_delta_k_um3"], 0.0);
    assert_eq!(
        null_result["curve"][0]["adjusted_interval_excludes_zero"],
        false
    );
    assert_eq!(
        null_result["curve"][0]["change_supported_beyond_registration"],
        false
    );

    let mismatched = directory.path().join("mismatched.json");
    fs::write(
        &mismatched,
        serde_json::to_vec_pretty(&fixture("patient-foreign")).unwrap(),
    )
    .unwrap();
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "registered-longitudinal-voxel-k-change",
            "--input",
            mismatched.to_str().unwrap(),
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
            "baseline and follow-up must belong to the declared patient",
        ));
}

#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "spatial3d-registered-serial-voxel-k",
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
fn registered_serial_volume_replays_with_missing_section_identity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let project = directory.path().join("project");
    let direct = directory.path().join("direct.json");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &input,
        br#"{"patient_id":"patient-1","specimen_id":"specimen-1","timepoint_id":"timepoint-1","anatomical_site":"colon-primary","volume_frame_id":"registered-volume-1","sections":[{"section_id":"section-0","ordinal":0,"z_center_um":0.5,"thickness_um":1.0,"status":"observed","source_frame_id":"section-0-xy","placement_coefficients":[1.0,0.0,0.0,0.0,1.0,0.0],"uncertainty":null,"points":[{"id":"a","coordinates_um":[0.5,0.5]}]},{"section_id":"section-1","ordinal":1,"z_center_um":1.5,"thickness_um":1.0,"status":"missing","source_frame_id":null,"placement_coefficients":null,"uncertainty":null,"points":[]},{"section_id":"section-2","ordinal":2,"z_center_um":2.5,"thickness_um":1.0,"status":"observed","source_frame_id":"section-2-xy","placement_coefficients":[1.0,0.0,1.0,0.0,1.0,0.0],"uncertainty":null,"points":[{"id":"b","coordinates_um":[0.5,0.5]}]}],"window":{"origin":[0.0,0.0,0.0],"voxel_size":[1.0,1.0,1.0],"dimensions":[2,1,3],"occupied_voxels":[[0,0,0],[1,0,2]],"maximum_grid_voxels":6},"anisotropy_matrix":null,"radii_um":[3.0],"correction":"none","maximum_unordered_pairs":1,"maximum_boundary_face_checks":100,"maximum_translation_voxel_pair_checks":1,"memory_budget_mib":16}"#,
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

    let direct_result: serde_json::Value =
        serde_json::from_slice(&fs::read(&direct).unwrap()).unwrap();
    let project_result: serde_json::Value =
        serde_json::from_slice(&fs::read(&first).unwrap()).unwrap();
    assert_eq!(direct_result, project_result);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(project_result["missing_section_count"], 1);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

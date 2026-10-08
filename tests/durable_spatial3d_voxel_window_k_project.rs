#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "spatial3d-voxel-k-function",
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
fn voxel_window_k_replays_without_recomputing_exact_window_overlap() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let project = directory.path().join("project");
    let direct = directory.path().join("direct.json");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &input,
        br#"{"points":[{"id":"a","coordinates":[0.5,0.5,0.5]},{"id":"b","coordinates":[1.5,0.5,0.5]}],"window":{"origin":[0.0,0.0,0.0],"voxel_size":[1.0,1.0,1.0],"dimensions":[2,2,1],"occupied_voxels":[[0,0,0],[1,0,0],[0,1,0]],"maximum_grid_voxels":4},"coordinate_unit":"micrometer","anisotropy_matrix":null,"radii_um":[1.0],"correction":"translation","maximum_unordered_pairs":1,"maximum_boundary_face_checks":100,"maximum_translation_voxel_pair_checks":9,"memory_budget_mib":16}"#,
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "voxel-k-function",
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
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

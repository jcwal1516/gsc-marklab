#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn voxel_window_k_handles_disconnected_geometry_and_an_enclosed_cavity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let l_input = directory.path().join("l-window.json");
    let l_output = directory.path().join("l-result.json");
    fs::write(
        &l_input,
        serde_json::to_vec_pretty(&serde_json::json!({
            "points": [
                {"id": "a", "coordinates": [0.5, 0.5, 0.5]},
                {"id": "b", "coordinates": [1.5, 0.5, 0.5]}
            ],
            "window": {
                "origin": [0.0, 0.0, 0.0],
                "voxel_size": [1.0, 1.0, 1.0],
                "dimensions": [2, 2, 1],
                "occupied_voxels": [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
                "maximum_grid_voxels": 4
            },
            "coordinate_unit": "micrometer",
            "anisotropy_matrix": null,
            "radii_um": [1.0],
            "correction": "translation",
            "maximum_unordered_pairs": 1,
            "maximum_boundary_face_checks": 100,
            "maximum_translation_voxel_pair_checks": 9,
            "memory_budget_mib": 16
        }))
        .unwrap(),
    )
    .expect("L fixture");

    run(&l_input, &l_output);
    let result: serde_json::Value = serde_json::from_slice(&fs::read(&l_output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.voxel_window_homogeneous_k3d");
    assert_eq!(result["window"]["occupied_voxel_count"], 3);
    assert_eq!(result["window"]["connected_components"], 1);
    assert_eq!(result["window"]["cavities"], 0);
    assert_eq!(result["window"]["volume_um3"], 3.0);
    assert_eq!(result["window"]["surface_area_um2"], 14.0);
    assert_eq!(result["curve"][0]["translation_overlap_sum_um3"], 1.0);
    assert_eq!(result["curve"][0]["k_um3"], 9.0);
    assert_eq!(result["translation_voxel_pair_checks"], 9);

    let shell_input = directory.path().join("shell-window.json");
    let shell_output = directory.path().join("shell-result.json");
    let occupied = (0..3)
        .flat_map(|x| (0..3).flat_map(move |y| (0..3).map(move |z| [x, y, z])))
        .filter(|voxel| *voxel != [1, 1, 1])
        .collect::<Vec<_>>();
    fs::write(
        &shell_input,
        serde_json::to_vec_pretty(&serde_json::json!({
            "points": [
                {"id": "a", "coordinates": [0.5, 0.5, 0.5]},
                {"id": "b", "coordinates": [2.5, 2.5, 2.5]}
            ],
            "window": {
                "origin": [0.0, 0.0, 0.0],
                "voxel_size": [1.0, 1.0, 1.0],
                "dimensions": [3, 3, 3],
                "occupied_voxels": occupied,
                "maximum_grid_voxels": 27
            },
            "coordinate_unit": "micrometer",
            "anisotropy_matrix": null,
            "radii_um": [4.0],
            "correction": "none",
            "maximum_unordered_pairs": 1,
            "maximum_boundary_face_checks": 1000,
            "maximum_translation_voxel_pair_checks": 1,
            "memory_budget_mib": 16
        }))
        .unwrap(),
    )
    .expect("shell fixture");

    run(&shell_input, &shell_output);
    let shell: serde_json::Value =
        serde_json::from_slice(&fs::read(shell_output).unwrap()).unwrap();
    assert_eq!(shell["window"]["connected_components"], 1);
    assert_eq!(shell["window"]["cavities"], 1);
    assert_eq!(shell["window"]["exposed_face_count"], 60);
    assert_eq!(shell["window"]["volume_um3"], 26.0);
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "voxel-k-function",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

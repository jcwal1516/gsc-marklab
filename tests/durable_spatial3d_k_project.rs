#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "spatial3d-k-function",
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
fn physical_cuboid_k_replays_with_the_translation_oracle_intact() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &input,
        br#"{"points":[{"id":"a","coordinates":[0.004,0.005,0.005]},{"id":"b","coordinates":[0.005,0.005,0.005]}],"window":{"minimum":[0.0,0.0,0.0],"maximum":[0.01,0.01,0.01]},"coordinate_unit":"millimeter","voxel_spacing":[0.001,0.001,0.001],"anisotropy_matrix":null,"radii_um":[0.5,1.0,2.0],"correction":"translation","maximum_unordered_pairs":10}"#,
    )
    .expect("fixture");

    project_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.homogeneous_k3d");
    assert_eq!(result["dimension"], 3);
    assert_eq!(result["window"]["volume_um3"], 1000.0);
    let k = result["curve"][1]["k_um3"].as_f64().unwrap();
    assert!((k - 10000.0 / 9.0).abs() <= 1.0e-10);
    assert_eq!(result["unordered_pairs_visited"], 1);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

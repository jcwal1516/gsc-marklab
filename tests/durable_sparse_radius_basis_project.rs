#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "sparse-radius-basis",
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
fn sparse_radius_basis_replays_without_repeating_the_eigensolver() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("basis.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let spec = serde_json::json!({
        "nodes": (0..5).map(|index| serde_json::json!({
            "id": format!("cell-{index}"),
            "coordinates_um": [index as f64, 0.0],
            "signal": 0.0
        })).collect::<Vec<_>>(),
        "radius_um": 1.1,
        "mode_count": 3,
        "maximum_iterations": 96,
        "residual_tolerance": 1e-9,
        "maximum_nodes": 5,
        "maximum_candidate_pairs": 10,
        "maximum_edges": 4,
        "maximum_components": 1,
        "maximum_matrix_vector_work": 10_000,
        "maximum_orthogonalization_work": 100_000,
        "maximum_ritz_rotations": 1_000,
        "maximum_working_bytes": 64_000,
        "maximum_retained_bytes": 64_000
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");

    command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.graph_sparse_radius_basis");
    assert_eq!(result["returned_mode_count"], 3);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

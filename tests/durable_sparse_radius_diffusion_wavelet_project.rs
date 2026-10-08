#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "sparse-radius-diffusion-wavelet",
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
fn sparse_diffusion_wavelet_replays_across_processes_without_recomputation() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("wavelet.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let signals = [1.0, -0.5, 2.0, 0.25, -1.0];
    let spec = serde_json::json!({
        "nodes": (0..5).map(|index| serde_json::json!({
            "id": format!("cell-{index}"),
            "coordinates_um": [index as f64, 0.0],
            "signal": signals[index]
        })).collect::<Vec<_>>(),
        "radius_um": 1.1,
        "times": [0.1, 0.2, 0.4],
        "tolerance": 1e-9,
        "maximum_order": 64,
        "maximum_nodes": 5,
        "maximum_candidate_pairs": 10,
        "maximum_edges": 4,
        "maximum_matrix_vector_work": 10_000,
        "maximum_working_bytes": 16_384,
        "maximum_retained_bytes": 1_000_000,
        "maximum_total_candidate_pairs": 30,
        "maximum_total_matrix_vector_work": 30_000
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");

    command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.graph_sparse_radius_diffusion_wavelet"
    );
    assert_eq!(result["scales"].as_array().unwrap().len(), 3);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

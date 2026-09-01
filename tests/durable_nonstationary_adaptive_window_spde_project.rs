#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(input: &Path, project: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "nonstationary-adaptive-window-spde",
        "--project",
        project.to_str().expect("project"),
        "--input",
        input.to_str().expect("input"),
        "--out",
        out.to_str().expect("output"),
    ]);
    command
}

#[test]
fn nonstationary_adaptive_spde_replays_without_a_second_backend_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("field.json");
    let mut observations = Vec::new();
    for row in 0..4 {
        for column in 0..4 {
            let x = 0.25 + 0.5 * column as f64;
            let y = 0.25 + 0.5 * row as f64;
            observations.push(serde_json::json!({
                "region_id": format!("r{row}_{column}"), "coordinates": [x,y],
                "values": [0.6*x+0.2*y,-0.4*x+0.1*y]
            }));
        }
    }
    let specification = serde_json::json!({
        "window_frame":"synthetic_square_um",
        "window":{"type":"MultiPolygon","coordinates":[[[[0.0,0.0],[2.0,0.0],[2.0,2.0],[0.0,2.0],[0.0,0.0]]]]},
        "base_resolution":6,"boundary_refinement_levels":1,"maximum_relative_area_error":0.15,
        "background":{"kappa":1.5,"tau":0.6,"anisotropy_ratio":1.0,"major_axis_degrees":0.0},
        "parameter_regions":[{"region_id":"focus","center":[0.6,1.0],"radius":0.45,"kappa":1.1,"tau":0.45,"anisotropy_ratio":2.0,"major_axis_degrees":30.0,"interior_refinement_levels":1}],
        "alpha":2,"factor_observations":observations,"factor_count":1,"factor_noise_sd":0.12,
        "maximum_iterations":1000,"maximum_vertices":256,"maximum_triangles":512,
        "maximum_candidate_points":1024,"maximum_boundary_segment_triangle_checks":200000,
        "maximum_projection_triangle_visits":200000,"memory_budget_mib":64,
        "maximum_result_bytes":2000000,"timeout_seconds":60
    });
    fs::write(
        &input,
        serde_json::to_vec_pretty(&specification).expect("input JSON"),
    )
    .expect("input");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    command(&input, &project, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&input, &project, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(
        fs::read(first).expect("miss"),
        fs::read(second).expect("hit")
    );
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .expect("ledger")
            .lines()
            .count(),
        1
    );
}

#![allow(dead_code)]

use std::{fs, path::Path};

use assert_cmd::Command;

pub fn write_input(root: &Path) {
    let mut observations = Vec::new();
    for row in 0..5 {
        for column in 0..5 {
            let x = 0.35 + 0.825 * column as f64;
            let y = 0.35 + 0.825 * row as f64;
            if (1.5..2.5).contains(&x) && (1.5..2.5).contains(&y) {
                continue;
            }
            let factor = 0.5 * x - 0.15 * y;
            observations.push(serde_json::json!({
                "region_id": format!("r{row}_{column}"),
                "coordinates": [x, y],
                "values": [factor, -0.6 * factor]
            }));
        }
    }
    let input = serde_json::json!({
        "window_frame": "synthetic_holed_square_um",
        "window": {
            "type": "MultiPolygon",
            "coordinates": [[
                [[0.0,0.0],[4.0,0.0],[4.0,4.0],[0.0,4.0],[0.0,0.0]],
                [[1.5,1.5],[1.5,2.5],[2.5,2.5],[2.5,1.5],[1.5,1.5]]
            ]]
        },
        "base_resolution": 7,
        "boundary_refinement_levels": 1,
        "maximum_relative_area_error": 0.12,
        "alpha": 2,
        "kappa": 1.5,
        "tau": 0.4,
        "factor_observations": observations,
        "factor_count": 1,
        "factor_noise_sd": 0.08,
        "maximum_iterations": 1000,
        "maximum_vertices": 256,
        "maximum_triangles": 512,
        "maximum_boundary_segment_triangle_checks": 200000,
        "maximum_projection_triangle_visits": 200000,
        "memory_budget_mib": 64,
        "maximum_result_bytes": 2000000,
        "timeout_seconds": 60
    });
    fs::write(
        root.join("adaptive-spde.json"),
        serde_json::to_vec_pretty(&input).expect("input JSON"),
    )
    .expect("input");
}

pub fn direct_command(root: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("marklab binary");
    command.args([
        "bayes",
        "adaptive-window-spde",
        "--input",
        root.join("adaptive-spde.json").to_str().expect("input"),
        "--out",
        out.to_str().expect("output"),
    ]);
    command
}

pub fn project_command(root: &Path, project: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("marklab binary");
    command.args([
        "project",
        "adaptive-window-spde",
        "--project",
        project.to_str().expect("project"),
        "--input",
        root.join("adaptive-spde.json").to_str().expect("input"),
        "--out",
        out.to_str().expect("output"),
    ]);
    command
}

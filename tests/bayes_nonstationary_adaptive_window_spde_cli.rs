#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn local_anisotropy_and_interior_refinement_change_one_holed_window_spde() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("field.json");
    let output = directory.path().join("result.json");
    let mut observations = Vec::new();
    for row in 0..6 {
        for column in 0..6 {
            let x = 0.3 + 0.68 * column as f64;
            let y = 0.3 + 0.68 * row as f64;
            if (1.5..2.5).contains(&x) && (1.5..2.5).contains(&y) {
                continue;
            }
            let field = if x < 2.0 {
                0.7 * x + 0.1 * y
            } else {
                0.2 * x + 0.8 * y
            };
            observations.push(serde_json::json!({
                "region_id": format!("r{row}_{column}"),
                "coordinates": [x, y],
                "values": [field, -0.5 * field]
            }));
        }
    }
    let specification = serde_json::json!({
        "window_frame": "synthetic_holed_square_um",
        "window": {"type":"MultiPolygon","coordinates":[[
            [[0.0,0.0],[4.0,0.0],[4.0,4.0],[0.0,4.0],[0.0,0.0]],
            [[1.5,1.5],[1.5,2.5],[2.5,2.5],[2.5,1.5],[1.5,1.5]]
        ]]},
        "base_resolution": 7,
        "boundary_refinement_levels": 1,
        "maximum_relative_area_error": 0.12,
        "background": {
            "kappa": 1.5, "tau": 0.6, "anisotropy_ratio": 1.0,
            "major_axis_degrees": 0.0
        },
        "parameter_regions": [{
            "region_id": "left_focus", "center": [0.9,2.0], "radius": 0.75,
            "kappa": 1.1, "tau": 0.45, "anisotropy_ratio": 4.0,
            "major_axis_degrees": 45.0, "interior_refinement_levels": 2
        }],
        "alpha": 2,
        "factor_observations": observations,
        "factor_count": 1,
        "factor_noise_sd": 0.12,
        "maximum_iterations": 1000,
        "maximum_vertices": 512,
        "maximum_triangles": 1024,
        "maximum_candidate_points": 2048,
        "maximum_boundary_segment_triangle_checks": 500000,
        "maximum_projection_triangle_visits": 500000,
        "memory_budget_mib": 128,
        "maximum_result_bytes": 4000000,
        "timeout_seconds": 60
    });
    fs::write(
        &input,
        serde_json::to_vec_pretty(&specification).expect("input JSON"),
    )
    .expect("input");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "nonstationary-adaptive-window-spde",
            "--input",
            input.to_str().expect("input path"),
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.nonstationary_adaptive_window_spde"
    );
    assert_eq!(result["window"]["hole_count"], 1);
    assert_eq!(
        result["mesh"]["operator"],
        "mass_lumped_nonstationary_alpha_two_spde"
    );
    assert!(
        result["mesh"]["minimum_precision_eigenvalue"]
            .as_f64()
            .expect("minimum eigenvalue")
            > 0.0
    );
    assert!(
        result["mesh"]["maximum_precision_symmetry_error"]
            .as_f64()
            .expect("symmetry")
            < 1e-10
    );
    let focus = result["parameter_regions"]
        .as_array()
        .expect("regions")
        .iter()
        .find(|region| region["region_id"] == "left_focus")
        .expect("focus");
    let tensor = focus["anisotropy_tensor"].as_array().expect("tensor");
    assert!((tensor[0][0].as_f64().unwrap() - 2.125).abs() < 1e-12);
    assert!((tensor[0][1].as_f64().unwrap() - 1.875).abs() < 1e-12);
    assert!((tensor[1][0].as_f64().unwrap() - 1.875).abs() < 1e-12);
    assert!((tensor[1][1].as_f64().unwrap() - 2.125).abs() < 1e-12);
    assert!(
        focus["median_triangle_area"].as_f64().expect("focus area")
            < result["background"]["median_triangle_area"]
                .as_f64()
                .expect("background area")
    );
    assert!(
        result["spatial_factor"]["gradient_maximum"]
            .as_f64()
            .expect("gradient")
            <= 2e-4
    );
    assert_eq!(
        result["statistical_unit"],
        "within_specimen_field_diagnostic"
    );
}

#![cfg(feature = "cli")]

use std::fs;

#[path = "support/adaptive_window_spde_fixture.rs"]
mod fixture;

#[test]
fn boundary_adaptive_spde_preserves_a_hole_and_matches_fem_integral_oracles() {
    let root = tempfile::tempdir().expect("root");
    fixture::write_input(root.path());
    let output = root.path().join("result.json");

    fixture::direct_command(root.path(), &output)
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(result["format"], "marklab.adaptive_window_spde");
    assert_eq!(result["version"], 1);
    assert_eq!(result["window"]["hole_count"], 1);
    assert_eq!(result["mesh"]["boundary_refinement_levels"], 1);
    assert_eq!(result["mesh"]["basis"], "piecewise_linear_triangular");
    assert_eq!(
        result["mesh"]["boundary_condition"],
        "natural_neumann_with_positive_kappa"
    );
    let exact_area = result["window"]["exact_area"].as_f64().expect("exact area");
    let mesh_area = result["mesh"]["mesh_area"].as_f64().expect("mesh area");
    let mass_integral = result["mesh"]["constant_mass_integral"]
        .as_f64()
        .expect("mass integral");
    assert!((exact_area - 15.0).abs() < 1e-12);
    assert!((mesh_area - mass_integral).abs() < 1e-10);
    assert!((mesh_area - exact_area).abs() / exact_area <= 0.12);
    assert!(
        result["mesh"]["minimum_precision_eigenvalue"]
            .as_f64()
            .expect("minimum eigenvalue")
            > 0.0
    );
    assert!(
        result["mesh"]["maximum_projection_row_sum_error"]
            .as_f64()
            .expect("projection error")
            < 1e-10
    );
    let vertices = result["mesh"]["vertices"].as_array().expect("vertices");
    for triangle in result["mesh"]["triangles"].as_array().expect("triangles") {
        let indices = triangle.as_array().expect("triangle");
        let centroid = indices
            .iter()
            .map(|index| &vertices[index.as_u64().expect("index") as usize])
            .fold([0.0, 0.0], |mut sum, vertex| {
                sum[0] += vertex[0].as_f64().expect("x") / 3.0;
                sum[1] += vertex[1].as_f64().expect("y") / 3.0;
                sum
            });
        assert!(
            !(1.5 < centroid[0] && centroid[0] < 2.5 && 1.5 < centroid[1] && centroid[1] < 2.5),
            "no retained element may cross the exact hole"
        );
    }
    let sensitivity = result["mesh_sensitivity"].as_array().expect("sensitivity");
    assert_eq!(sensitivity.len(), 2);
    assert!(
        sensitivity[1]["relative_area_error"].as_f64().unwrap()
            <= sensitivity[0]["relative_area_error"].as_f64().unwrap() + 1e-12
    );
    assert!(
        result["spatial_factor"]["factor_x_correlation"]
            .as_f64()
            .expect("factor correlation")
            > 0.8
    );
    assert!(
        result["spatial_factor"]["gradient_maximum"]
            .as_f64()
            .expect("gradient maximum")
            <= 2e-4
    );
    assert_eq!(
        result["claim_status"],
        "fitted_arbitrary_window_spde_diagnostic"
    );
}

#[test]
fn disconnected_components_produce_no_cross_component_precision_entries() {
    let root = tempfile::tempdir().expect("root");
    let mut observations = Vec::new();
    for (component, offset) in [("left", 0.0), ("right", 3.0)] {
        for row in 0..2 {
            for column in 0..2 {
                let x = offset + 0.4 + column as f64;
                let y = 0.4 + row as f64;
                observations.push(serde_json::json!({
                    "region_id": format!("{component}_{row}_{column}"),
                    "coordinates": [x, y],
                    "values": [x, -0.5 * x + 0.1 * y]
                }));
            }
        }
    }
    let input = serde_json::json!({
        "window_frame": "synthetic_disconnected_um",
        "window": {"type":"MultiPolygon","coordinates":[
            [[[0.0,0.0],[2.0,0.0],[2.0,2.0],[0.0,2.0],[0.0,0.0]]],
            [[[3.0,0.0],[5.0,0.0],[5.0,2.0],[3.0,2.0],[3.0,0.0]]]
        ]},
        "base_resolution": 7,
        "boundary_refinement_levels": 1,
        "maximum_relative_area_error": 0.2,
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
        root.path().join("adaptive-spde.json"),
        serde_json::to_vec_pretty(&input).expect("input"),
    )
    .expect("write input");
    let output = root.path().join("result.json");
    fixture::direct_command(root.path(), &output)
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(result["window"]["component_count"], 2);
    assert_eq!(result["mesh"]["connected_components"], 2);
    let vertices = result["mesh"]["vertices"].as_array().expect("vertices");
    let rows = result["mesh"]["precision_matrix"]["rows"]
        .as_array()
        .expect("precision rows");
    let columns = result["mesh"]["precision_matrix"]["columns"]
        .as_array()
        .expect("precision columns");
    for (row, column) in rows.iter().zip(columns) {
        let left_x = vertices[row.as_u64().expect("row") as usize][0]
            .as_f64()
            .expect("left x");
        let right_x = vertices[column.as_u64().expect("column") as usize][0]
            .as_f64()
            .expect("right x");
        assert_eq!(left_x < 2.5, right_x < 2.5);
    }
}

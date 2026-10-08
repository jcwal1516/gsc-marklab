#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn rectangular_spde_mesh_drives_lgcp_and_spatial_factor_inference_with_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("spde.json");
    let mut points = Vec::new();
    for index in 0..60 {
        let x = if index < 45 {
            0.55 + 0.4 * ((index * 17 % 43) as f64 / 43.0)
        } else {
            0.05 + 0.4 * ((index * 11 % 41) as f64 / 41.0)
        };
        let y = 0.05 + 0.9 * ((index * 13 % 59) as f64 / 59.0);
        points.push(serde_json::json!([x, y]));
    }
    let mut observations = Vec::new();
    for row in 0..6 {
        for column in 0..6 {
            let x = 0.08 + 0.168 * column as f64;
            let y = 0.08 + 0.168 * row as f64;
            let factor = 2.0 * x - 1.0 + 0.2 * (std::f64::consts::PI * y).sin();
            observations.push(serde_json::json!({
                "region_id":format!("r{row}_{column}"),"coordinates":[x,y],
                "values":[factor + 0.01 * row as f64,-0.5 * factor + 0.005 * column as f64]
            }));
        }
    }
    let fixture = serde_json::json!({
        "window":{"frame":"synthetic_unit_square","bounds":[0.0,1.0,0.0,1.0],"holes":[]},
        "mesh_resolutions":[5,7],"alpha":2,"kappa":3.0,"tau":0.2,
        "lgcp_points":points,"lgcp_beta_prior_sd":3.0,
        "factor_observations":observations,"factor_count":1,"factor_noise_sd":0.08,
        "maximum_iterations":1000,"timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "spde-suite",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.rectangular_spde_suite");
    assert_eq!(result["mesh"]["vertex_count"], 49);
    assert_eq!(result["mesh"]["triangle_count"], 72);
    assert!(
        result["mesh"]["minimum_precision_eigenvalue"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["mesh"]["maximum_projection_row_sum_error"]
            .as_f64()
            .unwrap()
            < 1e-12
    );
    assert!(
        result["lgcp"]["right_to_left_mean_intensity_ratio"]
            .as_f64()
            .unwrap()
            > 1.5
    );
    assert!((result["lgcp"]["integrated_intensity"].as_f64().unwrap() - 60.0).abs() < 3.0);
    assert!(
        result["spatial_factor"]["reconstruction_rmse"]
            .as_f64()
            .unwrap()
            < 0.12
    );
    assert!(
        result["spatial_factor"]["factor_x_correlation"]
            .as_f64()
            .unwrap()
            > 0.9
    );
    assert_eq!(result["mesh_sensitivity"].as_array().unwrap().len(), 2);
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_rectangular_spde_lgcp_and_factor_models"
    );
}

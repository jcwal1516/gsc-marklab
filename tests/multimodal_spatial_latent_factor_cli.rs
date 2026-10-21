#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pymc_matern_spatial_factor_recovers_masked_shared_field_values() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("spatial_factor.json");
    let rows = (0..12)
        .map(|index| {
            let x = index as f64 / 11.0;
            let latent = (std::f64::consts::PI * x).sin() - 0.4 * x;
            let values = [latent, 1.7 * latent];
            let observed = (0..2)
                .map(|feature| !matches!((index, feature), (3, 0) | (7, 1) | (10, 0)))
                .collect::<Vec<_>>();
            serde_json::json!({
                "entity_id":format!("r{index:02}"),
                "coordinates_um":[100.0*x,0.0],
                "values":values,
                "observed":observed
            })
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "matrix_id":"region_spatial_features",
        "entity_level":"region",
        "coordinate_frame":"slide_um",
        "likelihood":"gaussian",
        "feature_names":["morphology","ihc"],
        "rows":rows,
        "factors":1,
        "matern_nu":1.5,
        "length_scale_prior_um":60.0,
        "noise_standard_deviation":0.12,
        "warmup":150,
        "samples":100,
        "target_accept":0.99,
        "seed":37,
        "timeout_seconds":90
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "spatial-latent-factor",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.spatial_latent_factor_model");
    assert_eq!(result["backend"]["version"], "pymc-6.3.0");
    assert_eq!(result["masked_predictions"].as_array().unwrap().len(), 3);
    assert!(result["masked_rmse"].as_f64().unwrap() < 0.35);
    assert!(
        result["posterior_length_scale_um"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["alignment"]["sign"], "max_loading_positive_per_draw");
}

fn fit_two_dimensional(latent_field: Option<serde_json::Value>, nu: f64) -> serde_json::Value {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("spatial_factor.json");
    let rows = (0..36)
        .map(|index| {
            let jitter = |k: usize| ((k * 37) % 17) as f64 - 8.0;
            let x = 40.0 + 84.0 * (index % 6) as f64 + jitter(index);
            let y = 40.0 + 84.0 * (index / 6) as f64 + jitter(index * 5 + 1);
            let latent = truth(x, y);
            let observed = (0..3)
                .map(|feature| !matches!((index, feature), (4, 0) | (13, 1) | (22, 2) | (31, 0)))
                .collect::<Vec<_>>();
            serde_json::json!({
                "entity_id": format!("r{index:02}"),
                "coordinates_um": [x, y],
                "values": [latent, 1.5 * latent, -0.8 * latent],
                "observed": observed
            })
        })
        .collect::<Vec<_>>();
    let mut fixture = serde_json::json!({
        "matrix_id": "region_spatial_features", "entity_level": "region",
        "coordinate_frame": "slide_um", "likelihood": "gaussian",
        "feature_names": ["morphology", "ihc", "density"], "rows": rows, "factors": 1,
        "matern_nu": nu, "length_scale_prior_um": 300.0, "noise_standard_deviation": 0.12,
        "warmup": 200, "samples": 150, "target_accept": 0.95, "seed": 41,
        "timeout_seconds": 600
    });
    if let Some(field) = latent_field {
        fixture["latent_field"] = field;
    }
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "spatial-latent-factor",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON")
}

fn truth(x: f64, y: f64) -> f64 {
    (std::f64::consts::PI * x / 500.0).sin() + 0.5 * (std::f64::consts::PI * y / 400.0).cos()
}

fn correlation(left: &[f64], right: &[f64]) -> f64 {
    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
    let (left_mean, right_mean) = (mean(left), mean(right));
    let covariance = left
        .iter()
        .zip(right)
        .map(|(a, b)| (a - left_mean) * (b - right_mean))
        .sum::<f64>();
    let spread = |values: &[f64], center: f64| {
        values
            .iter()
            .map(|value| (value - center).powi(2))
            .sum::<f64>()
            .sqrt()
    };
    covariance / (spread(left, left_mean) * spread(right, right_mean))
}

#[test]
fn spde_spatial_factor_recovers_the_field_found_by_the_dense_gp() {
    let dense = fit_two_dimensional(None, 1.5);
    let spde = fit_two_dimensional(
        Some(
            serde_json::json!({"kind": "spde_alpha_two", "mesh_resolution": 10,
            "mesh_extension_um": 200.0}),
        ),
        1.0,
    );
    assert_eq!(spde["kernel"]["family"], "spde_matern");
    assert_eq!(spde["kernel"]["mesh_vertices"], 100);
    assert!(spde["posterior_range_um"]["mean"].as_f64().unwrap() > 0.0);
    let factor = |result: &serde_json::Value| {
        result["posterior_spatial_factor"]["mean"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect::<Vec<_>>()
    };
    let expected = (0..36)
        .map(|index| {
            let jitter = |k: usize| ((k * 37) % 17) as f64 - 8.0;
            truth(
                40.0 + 84.0 * (index % 6) as f64 + jitter(index),
                40.0 + 84.0 * (index / 6) as f64 + jitter(index * 5 + 1),
            )
        })
        .collect::<Vec<_>>();
    // Thresholds fixed before either fit was run.
    assert!(correlation(&factor(&spde), &expected).abs() >= 0.9);
    assert!(correlation(&factor(&dense), &expected).abs() >= 0.9);
    assert!(correlation(&factor(&spde), &factor(&dense)) >= 0.9);
}

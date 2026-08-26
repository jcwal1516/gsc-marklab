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

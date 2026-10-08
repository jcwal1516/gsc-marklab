#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn graph_laplacian_spatial_matrix_factor_recovers_masked_smooth_entries() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("spatial_matrix.json");
    let rows = (0..16)
        .map(|index| {
            let u = (index as f64 - 7.5) / 4.0;
            let values = [u, 1.5 * u, -u, 0.5 * u];
            let observed = (0..4)
                .map(|feature| !matches!((index, feature), (4, 0) | (7, 1) | (10, 2) | (13, 3)))
                .collect::<Vec<_>>();
            serde_json::json!({"entity_id":format!("r{index:02}"),"values":values,"observed":observed})
        })
        .collect::<Vec<_>>();
    let edges = (0..15)
        .map(|index| serde_json::json!({"left":format!("r{index:02}"),"right":format!("r{:02}",index+1),"weight":1.0}))
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "matrix_id":"region_features",
        "entity_level":"region",
        "likelihood":"gaussian",
        "feature_names":["f1","f2","f3","f4"],
        "rows":rows,
        "graph":{"graph_id":"region_path","edges":edges,"normalization":"unnormalized_laplacian"},
        "factors":2,
        "spatial_precision":0.2,
        "diagonal_epsilon":0.01,
        "loading_precision":0.1,
        "noise_standard_deviation":0.1,
        "maximum_iterations":500,
        "seed":29,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "spatial-matrix-factor",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.spatial_bayesian_matrix_factorization"
    );
    assert_eq!(result["backend"]["version"], "scipy-1.18.1");
    assert_eq!(result["masked_predictions"].as_array().unwrap().len(), 4);
    assert!(result["masked_rmse"].as_f64().unwrap() < 0.35);
    assert!(result["masked_predictions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|prediction| { prediction["posterior_standard_deviation"].as_f64().unwrap() > 0.0 }));
    assert_eq!(
        result["spatial_prior"]["operator"],
        "graph_laplacian_plus_diagonal"
    );
    assert_eq!(result["fit_state"], "approximate_only");
}

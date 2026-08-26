#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pinned_bayesian_pcca_recovers_shared_view_predictions_with_aligned_draws() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("bayesian-pcca.json");
    let rows = (0..16)
        .map(|index| {
            let z = (index as f64 - 7.5) / 2.0;
            let noise = match index % 4 {
                0 => -0.06,
                1 => 0.04,
                2 => 0.06,
                _ => -0.04,
            };
            serde_json::json!({
                "entity_id": format!("p{index:02}"),
                "split": if index < 12 { "train" } else { "test" },
                "x": [z + noise, 0.6 * z - noise],
                "y": [1.7 * z - noise, -0.8 * z + noise]
            })
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "design": {
            "entity_level": "patient",
            "modality_x": {"id":"morphology","measurement_status":"measured","likelihood":"gaussian","feature_names":["x1","x2"]},
            "modality_y": {"id":"ihc","measurement_status":"measured","likelihood":"gaussian","feature_names":["y1","y2"]},
            "missingness_assumption":"complete_paired_rows",
            "coordinate_frame": null
        },
        "rows": rows,
        "latent_dimensions":1,
        "priors":"cca_zoo_standard_normal_loadings_log_noise",
        "warmup":100,
        "samples":80,
        "target_accept":0.99,
        "seed":71,
        "timeout_seconds":120
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "bayesian-pcca",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_pcca");
    assert_eq!(result["backend"]["cca_zoo_version"], "3.0.0");
    assert_eq!(result["design"]["validation_status"], "passed");
    assert_eq!(result["standardization"]["fit_split"], "train_only");
    assert_eq!(
        result["alignment"]["method"],
        "first_view_max_loading_positive"
    );
    assert_eq!(result["alignment"]["aligned_draw_count"], 80);
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert!(
        result["posterior_mean_canonical_correlation"]
            .as_f64()
            .unwrap()
            > 0.95
    );
    assert!(result["heldout_cross_view_rmse_y_from_x"].as_f64().unwrap() < 0.4);
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_bayesian_pcca"
    );
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn bayesian_landmark_posterior_propagates_uncertainty_and_recovers_soft_matches() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("landmark_uncertainty.json");
    let fixture = serde_json::json!({
        "source_frame":"source_slide_um",
        "target_frame":"target_slide_um",
        "source_landmarks":[[0.0,0.0],[10.0,0.0],[0.0,10.0],[10.0,10.0]],
        "target_landmarks":[[2.0,-1.0],[12.0,-1.0],[2.0,9.0],[12.0,9.0]],
        "source_cells":[
            {"cell_id":"s1","coordinates_um":[2.0,2.0],"features":[0.0]},
            {"cell_id":"s2","coordinates_um":[5.0,5.0],"features":[1.0]},
            {"cell_id":"s3","coordinates_um":[8.0,7.0],"features":[2.0]}
        ],
        "target_cells":[
            {"cell_id":"t1","coordinates_um":[4.0,1.0],"features":[0.0]},
            {"cell_id":"t2","coordinates_um":[7.0,4.0],"features":[1.0]},
            {"cell_id":"t3","coordinates_um":[10.0,6.0],"features":[2.0]},
            {"cell_id":"outlier","coordinates_um":[30.0,30.0],"features":[8.0]}
        ],
        "deformation_prior":{"kernel":"squared_exponential","amplitude_um":4.0,"length_scale_um":20.0},
        "landmark_noise_standard_deviation_um":0.1,
        "localization_standard_deviation_um":0.05,
        "posterior_draws":256,
        "spatial_cost_scale_um":1.0,
        "feature_cost_scale":0.5,
        "dustbin_cost":5.0,
        "seed":67,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "registration",
            "landmark-uncertainty",
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
        "marklab.landmark_uncertainty_and_correspondence"
    );
    assert_eq!(result["transform_posterior"]["draw_count"], 256);
    assert!(
        result["quality"]["landmark_posterior_mean_rmse_um"]
            .as_f64()
            .unwrap()
            < 0.05
    );
    assert!(
        result["downstream_uncertainty"]["monte_carlo_standard_deviation"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["downstream_uncertainty"]["delta_standard_deviation"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["downstream_uncertainty"]["delta_vs_monte_carlo_relative_error"]
            .as_f64()
            .unwrap()
            < 0.25
    );
    let correspondences = result["correspondences"].as_array().unwrap();
    for (index, target_id) in ["t1", "t2", "t3"].iter().enumerate() {
        let probabilities = correspondences[index]["probabilities"].as_array().unwrap();
        let matched = probabilities
            .iter()
            .find(|item| item["target_id"] == *target_id)
            .unwrap();
        assert!(matched["probability"].as_f64().unwrap() > 0.9);
    }
    assert_eq!(
        result["claim_status"],
        "probabilistic_compatibility_not_cell_identity"
    );
}

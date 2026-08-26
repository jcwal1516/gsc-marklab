#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn gaussian(shift_x: f64, shift_y: f64) -> Vec<Vec<f64>> {
    (0..24)
        .map(|y| {
            (0..24)
                .map(|x| {
                    let dx = x as f64 - (11.0 + shift_x);
                    let dy = y as f64 - (12.0 + shift_y);
                    (-0.5 * (dx * dx + 1.4 * dy * dy) / 10.0).exp()
                })
                .collect()
        })
        .collect()
}

#[test]
fn stationary_velocity_scaling_and_squaring_recovers_translation_diffeomorphism() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("svf.json");
    let fixture = serde_json::json!({
        "fixed":{"frame":"fixed_slide_um","spacing_um":[1.0,1.0],"pixels":gaussian(0.0,0.0)},
        "moving":{"frame":"moving_slide_um","spacing_um":[1.0,1.0],"pixels":gaussian(2.0,-1.0)},
        "metric":"mean_squares_same_stain",
        "regularization_weight":0.02,
        "squaring_steps":4,
        "maximum_iterations":100,
        "jacobian_tolerance":0.2,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "registration",
            "svf",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.svf_diffeomorphic_registration");
    assert!(
        result["quality"]["mse_after"].as_f64().unwrap()
            < result["quality"]["mse_before"].as_f64().unwrap() * 0.15
    );
    assert!(
        result["quality"]["minimum_jacobian_determinant"]
            .as_f64()
            .unwrap()
            > 0.8
    );
    assert!(
        result["quality"]["inverse_consistency_max_pixels"]
            .as_f64()
            .unwrap()
            < 0.05
    );
    assert_eq!(
        result["transform"]["exponentiation"],
        "scaling_and_squaring"
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_svf_diffeomorphism"
    );
}

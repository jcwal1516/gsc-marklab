#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn gaussian_image(shift_x: f64, shift_y: f64) -> Vec<Vec<f64>> {
    (0..32)
        .map(|y| {
            (0..32)
                .map(|x| {
                    let dx = x as f64 - (15.0 + shift_x);
                    let dy = y as f64 - (16.0 + shift_y);
                    (-0.5 * (dx * dx / 20.0 + dy * dy / 12.0)).exp()
                })
                .collect()
        })
        .collect()
}

#[test]
fn pinned_simpleitk_multiresolution_bspline_reduces_known_shift_error() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("registration.json");
    let fixture = serde_json::json!({
        "fixed":{"frame":"fixed_slide_um","spacing_um":[1.0,1.0],"pixels":gaussian_image(0.0,0.0)},
        "moving":{"frame":"moving_slide_um","spacing_um":[1.0,1.0],"pixels":gaussian_image(2.0,-1.0)},
        "fixed_mask":vec![vec![true;32];32],
        "moving_mask":vec![vec![true;32];32],
        "metric":"mean_squares_same_stain",
        "mesh_size":[4,4],
        "shrink_factors":[2,1],
        "smoothing_sigmas_um":[1.0,0.0],
        "maximum_iterations":100,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "registration",
            "nonrigid",
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
        "marklab.multiresolution_nonrigid_registration"
    );
    assert_eq!(result["backend"]["version"], "SimpleITK-2.5.5");
    assert!(
        result["quality"]["masked_mse_after"].as_f64().unwrap()
            < result["quality"]["masked_mse_before"].as_f64().unwrap() * 0.2
    );
    assert!(
        result["quality"]["minimum_jacobian_determinant"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["transform"]["type"], "bspline");
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_nonrigid_registration"
    );
}

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
                    (-0.5 * (dx * dx + dy * dy) / 9.0).exp()
                })
                .collect()
        })
        .collect()
}

#[test]
fn variational_translation_svf_calibrates_known_deformation_draws() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("probabilistic_svf.json");
    let fixture = serde_json::json!({
        "fixed":{"frame":"fixed_slide_um","spacing_um":[1.0,1.0],"pixels":gaussian(0.0,0.0)},
        "moving":{"frame":"moving_slide_um","spacing_um":[1.0,1.0],"pixels":gaussian(2.0,-1.0)},
        "velocity_family":"constant_translation_svf",
        "prior_standard_deviation_pixels":4.0,
        "likelihood_noise_standard_deviation":0.03,
        "variational_samples":16,
        "posterior_draws":64,
        "maximum_iterations":500,
        "seed":61,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "registration",
            "probabilistic-svf",
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
        "marklab.probabilistic_diffeomorphic_registration"
    );
    assert_eq!(
        result["posterior_transform_draws"]
            .as_array()
            .unwrap()
            .len(),
        64
    );
    let mean = result["posterior_velocity_xy_pixels"]["mean"]
        .as_array()
        .unwrap();
    assert!((mean[0].as_f64().unwrap() - 2.0).abs() < 0.25);
    assert!((mean[1].as_f64().unwrap() + 1.0).abs() < 0.25);
    assert_eq!(
        result["calibration"]["known_translation_inside_95_percent_interval"],
        true
    );
    assert!(
        result["quality"]["minimum_sample_jacobian_determinant"]
            .as_f64()
            .unwrap()
            > 0.99
    );
    assert!(
        result["diagnostics"]["elbo_final"].as_f64().unwrap()
            > result["diagnostics"]["elbo_initial"].as_f64().unwrap()
    );
}

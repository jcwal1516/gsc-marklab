#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn deformation_biology_model_separates_translation_from_known_domain_change() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("deformation_biology.json");
    let mut points = Vec::new();
    for iy in 0..6 {
        for ix in 0..6 {
            let x = ix as f64 / 5.0;
            let y = iy as f64 / 5.0;
            let baseline = (std::f64::consts::PI * x).sin() + (std::f64::consts::PI * y).cos();
            let domain = if x > 0.5 { 1.0 } else { 0.0 };
            points.push(serde_json::json!({
                "point_id":format!("q{iy}{ix}"),"pre_coordinates":[x,y],"pre_value":baseline,
                "post_coordinates":[x+0.1,y-0.05],"post_value":baseline+domain,
                "negative_control_post_value":baseline,"domain_indicator":domain,"independent_change_measurement":domain
            }));
        }
    }
    let draws = (0..128)
        .map(|index| {
            let jitter = 0.004 * ((index as f64) * 0.7).sin();
            [0.1 + jitter, -0.05 - 0.5 * jitter]
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "coordinate_frame":"longitudinal_unit_square",
        "points":points,
        "deformation_translation_draws":draws,
        "change_model":"intercept_plus_prespecified_domain",
        "interpolation":"thin_plate_spline",
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "deformation-biology",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.deformation_biology_model");
    assert!(
        (result["biological_change"]["domain_effect_mean"]
            .as_f64()
            .unwrap()
            - 1.0)
            .abs()
            < 0.1
    );
    assert!(
        result["biological_change"]["domain_effect_standard_deviation"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["negative_control"]["absolute_domain_effect_mean"]
            .as_f64()
            .unwrap()
            < 0.1
    );
    assert!(
        result["independent_measurement"]["correlation"]
            .as_f64()
            .unwrap()
            > 0.95
    );
    assert!(
        result["deformation"]["translation_xy_mean"][0]
            .as_f64()
            .unwrap()
            > 0.09
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_deformation_biology_separation"
    );
}

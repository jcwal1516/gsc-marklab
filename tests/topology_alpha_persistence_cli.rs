#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn equilateral_triangle_alpha_persistence_and_transforms_match_oracles() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("alpha.json");
    let height = 3.0_f64.sqrt();
    fs::write(
        &input,
        format!(
            r#"{{
  "points":[
    {{"id":"a","coordinates_um":[0.0,0.0]}},
    {{"id":"b","coordinates_um":[2.0,0.0]}},
    {{"id":"c","coordinates_um":[1.0,{height}]}}
  ],
  "max_alpha_um":2.0,
  "maximum_dimension":2,
  "coefficient_field":2,
  "transform_dimension":1,
  "landscape_grid_alpha_squared":[1.0,1.1666666666666667,1.3333333333333333],
  "landscape_max_k":1,
  "image_birth_edges_alpha_squared":[0.5,1.5],
  "image_persistence_edges_alpha_squared":[0.0,0.5],
  "image_kernel_bandwidth_alpha_squared":0.1,
  "euler_thresholds_alpha_squared":[0.0,1.0,1.3333333333333333],
  "maximum_simplices":100,
  "timeout_seconds":30
}}"#
        ),
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "topology",
            "alpha-persistence",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.alpha_persistence");
    assert_eq!(result["backend"]["name"], "gudhi");
    assert_eq!(result["backend"]["version"], "3.13.0");
    assert_eq!(result["filtration"]["validation_status"], "passed");
    assert_eq!(
        result["filtration"]["simplex_counts_by_dimension"],
        serde_json::json!([3, 3, 1])
    );
    assert!(
        result["filtration"]["boundary_of_boundary_max_abs"]
            .as_f64()
            .unwrap()
            < 1e-12
    );

    let dimension_zero = &result["persistence"]["by_dimension"][0];
    assert_eq!(dimension_zero["essential_count"], 1);
    let finite_zero = dimension_zero["finite_pairs"].as_array().unwrap();
    assert_eq!(finite_zero.len(), 2);
    assert!(finite_zero.iter().all(|pair| {
        pair["birth"].as_f64().unwrap().abs() < 1e-12
            && (pair["death"].as_f64().unwrap() - 1.0).abs() < 1e-12
    }));
    let dimension_one = &result["persistence"]["by_dimension"][1];
    assert_eq!(dimension_one["finite_pairs"].as_array().unwrap().len(), 1);
    let hole = &dimension_one["finite_pairs"][0];
    assert!((hole["birth"].as_f64().unwrap() - 1.0).abs() < 1e-12);
    assert!((hole["death"].as_f64().unwrap() - 4.0 / 3.0).abs() < 1e-12);

    let landscape = result["landscape"]["values"][0].as_array().unwrap();
    for (actual, expected) in landscape.iter().zip([0.0, 1.0 / 6.0, 0.0]) {
        assert!((actual.as_f64().unwrap() - expected).abs() < 1e-12);
    }
    let image_mass = result["persistence_image"]["values"][0][0]
        .as_f64()
        .unwrap();
    assert!(image_mass > 0.30 && image_mass < 1.0 / 3.0);
    assert_eq!(
        result["euler_curve"]["values"],
        serde_json::json!([3, 0, 1])
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_alpha_topology"
    );
}

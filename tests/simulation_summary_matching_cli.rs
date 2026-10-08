#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn soft_pair_histogram_matches_the_gaussian_oracle_and_is_permutation_invariant() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let mut specification = serde_json::json!({
        "window": {"xmin_um": 0.0, "ymin_um": 0.0, "xmax_um": 3.0, "ymax_um": 3.0},
        "observed": [{"point_id": "o1", "x_um": 0.0, "y_um": 0.0}, {"point_id": "o2", "x_um": 1.0, "y_um": 0.0}],
        "generated": [{"point_id": "g1", "x_um": 0.0, "y_um": 0.0}, {"point_id": "g2", "x_um": 1.5, "y_um": 0.0}],
        "radius_centers_um": [1.0, 2.0],
        "summary_weights": [1.0, 2.0],
        "bandwidth_um": 0.5,
        "normalization": "pair_probability_density",
        "maximum_pair_bin_visits": 100
    });
    fs::write(&input, serde_json::to_vec(&specification).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    run(&input, &output);
    let result: serde_json::Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    let peak = 1.0 / (0.5 * (2.0 * std::f64::consts::PI).sqrt());
    assert!((result["observed"]["bins"][0]["value"].as_f64().unwrap() - peak).abs() < 1e-12);
    assert!(result["loss"]["total"].as_f64().unwrap() > 0.0);
    assert_eq!(result["observed"]["pair_count"], 1);
    assert_eq!(result["generated"]["pair_count"], 1);
    assert_eq!(
        result["differentiability"],
        "analytic_in_pair_distances_away_from_coincident_points"
    );

    specification["generated"] = specification["observed"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .into();
    fs::write(&input, serde_json::to_vec(&specification).unwrap()).unwrap();
    run(&input, &output);
    let identical: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert!(identical["loss"]["total"].as_f64().unwrap().abs() < 1e-15);
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "simulate",
            "summary-matching",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn stable_primitives_match_overflow_and_cancellation_oracles() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "log_values": [1000.0, 999.0],
            "weighted_values": [1.0e16, 1.0, -1.0e16],
            "weights": [1.0, 1.0, 1.0],
            "matrix": [[1.0, 2.0], [2.0, 4.0], [3.0, 6.0]],
            "covariance_weights": null,
            "maximum_elements": 100,
            "maximum_covariance_cross_products": 100
        }))
        .unwrap(),
    )
    .unwrap();
    let output = directory.path().join("output.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "numerics",
            "stable-primitives",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.stable_numerical_primitives");
    assert_close(
        result["log_sum_exp"].as_f64().unwrap(),
        1000.0 + (-1.0_f64).exp().ln_1p(),
    );
    assert_close(
        result["log_mean_exp"].as_f64().unwrap(),
        1000.0 + (-1.0_f64).exp().ln_1p() - 2.0_f64.ln(),
    );
    assert_close(result["weighted_mean"].as_f64().unwrap(), 1.0 / 3.0);
    assert_eq!(
        result["covariance"],
        serde_json::json!([[1.0, 2.0], [2.0, 4.0]])
    );
    assert_eq!(result["covariance_effective_sample_size"], 3.0);
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-12,
        "{actual} != {expected}"
    );
}
